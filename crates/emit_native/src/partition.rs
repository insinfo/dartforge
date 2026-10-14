//! The program module in parts (docs/NATIVO-PROJETOS-REAIS.md, C9).
//!
//! The IR of a large program does not fit in one Clang: `-O0` spends ~44 bytes
//! of memory per byte of IR (24.8 MB of IR, 1.08 GB peak, measured), and
//! new_sali/backend has 1.3 GB of IR — 55 GB for a single process. Here the
//! text of the module is split into parts of ~[`DEFAULT_TARGET`] bytes, each one
//! a complete LLVM module, compiled one by one (or a few at a time) and
//! linked together. Each part has its own key in the object cache, so an
//! edit recompiles only the parts that changed.
//!
//! What the emitter writes (`llvm/mod.rs`) and how each thing goes to the parts:
//!
//! * `define … @f(…) { … }` and `@g = …` with external or
//!   `linkonce_odr` linkage: in a single part; the others that cite them get the
//!   declaration (`declare` with the parameter types, or
//!   `@g = external global|constant <ty>`);
//! * `private`/`internal` constants and functions (the `@.str.N` strings, the
//!   vectors, the `@df.*` `alwaysinline` helpers): copied into each part
//!   that cites them, with what they cite (transitive closure). They are immutable, and
//!   two private copies do not see each other;
//! * a mutable `internal global` (the `@df.area_id`, the index of the module's
//!   global area, and the thread-local variable of the context,
//!   `sdk_module::with_context_in_tls`): one copy per part would break the
//!   state — it gets external linkage in part 0 and is declared in the
//!   others (the thread-local one, `thread_local`);
//! * `declare`, the header (`target`, `source_filename`), attributes and
//!   metadata: in every part; `$x = comdat any`, in the part of the item that uses it;
//!   the comments (`; df.class`, `; df.fields`, which the JIT reload reads
//!   from the whole IR) only in part 0.
//!
//! The parts group the items by the library of the symbol
//! (`df.<library>.…`), in order, so that the content of a part depends on
//! its libraries.

use std::collections::{HashMap, HashSet};

/// IR size from which the module is split.
pub const DEFAULT_THRESHOLD: usize = 48 << 20;
/// Size of each part.
pub const DEFAULT_TARGET: usize = 16 << 20;

/// The threshold and the target, adjustable (`DARTFORGE_PART_MB`, in MiB; `0` turns
/// splitting off).
pub fn limits() -> Option<(usize, usize)> {
    match std::env::var("DARTFORGE_PART_MB")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        Some(0) => None,
        Some(mb) => Some((2 * (mb << 20), mb << 20)),
        None => Some((DEFAULT_THRESHOLD, DEFAULT_TARGET)),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ItemKind {
    Function,
    Global,
}

/// An item defined in the module.
struct Item<'a> {
    name: &'a str,
    ty: ItemKind,
    text: &'a str,
    local: bool,
    /// `internal global` (not `constant`): state, it cannot be copied.
    mutable: bool,
    comdat: Option<&'a str>,
}

/// The module as read.
struct Module<'a> {
    header: Vec<&'a str>,
    comments: Vec<&'a str>,
    declarations: Vec<(&'a str, &'a str)>,
    comdats: HashMap<&'a str, &'a str>,
    items: Vec<Item<'a>>,
    index: HashMap<&'a str, usize>,
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'$' | b'-')
}

/// The name in `@name…` starting at `s` (which starts after the `@`) and how many
/// bytes it takes in the text: `@"…"` (the emitter quotes names with `$` this way
/// in a reference, `@"df.ffi.S1$p_$ent"`) counts as the name without the quotes, to
/// match the definition written without them.
fn name_and_width(s: &str) -> (&str, usize) {
    let b = s.as_bytes();
    if b.first() == Some(&b'"') {
        return s[1..].find('"').map_or(("", 0), |f| (&s[1..f + 1], f + 2));
    }
    let end = b.iter().position(|c| !is_ident(*c)).unwrap_or(b.len());
    (&s[..end], end)
}

fn name_in(s: &str) -> &str {
    name_and_width(s).0
}

/// Locate a function identifier before its parameter list. Parentheses and
/// `@` bytes inside a quoted identifier are part of the name, not delimiters.
fn function_identifier(line: &str) -> Option<(usize, usize)> {
    let mut position = 0;
    let bytes = line.as_bytes();
    while position < bytes.len() {
        match bytes[position] {
            b'"' => {
                let width = name_and_width(&line[position..]).1;
                if width == 0 {
                    return None;
                }
                position += width;
            }
            b'@' => {
                let width = name_and_width(&line[position + 1..]).1;
                return (width != 0).then_some((position, width));
            }
            _ => position += 1,
        }
    }
    None
}

/// All the `@name` references of a text.
fn references<'a>(text: &'a str, exit: &mut Vec<&'a str>) {
    let b = text.as_bytes();
    let mut i = 0;
    while let Some(p) = memchr_at(b, b'@', i) {
        let (n, width) = name_and_width(&text[p + 1..]);
        if !n.is_empty() {
            exit.push(n);
        }
        i = p + 1 + width;
    }
}

fn memchr_at(b: &[u8], c: u8, of: usize) -> Option<usize> {
    b.get(of..)?.iter().position(|x| *x == c).map(|p| p + of)
}

/// The first line of a text, with the line break.
fn first_line(t: &str) -> &str {
    t.find(char::from(10)).map_or(t, |i| &t[..=i])
}

/// `$x`/`$"x"` → `x`.
fn comdat_name(s: &str) -> &str {
    let s = s.strip_prefix('$').unwrap_or(s);
    s.strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(s)
}

/// The comdat of a `define`/global: an explicit `comdat($x)`, or just `comdat`
/// (the one with the same name as the item — the static objects of production,
/// `@df.img.…`).
fn line_comdat<'a>(line: &'a str, name: &'a str) -> Option<&'a str> {
    if let Some(c) = line.find("comdat($") {
        let r = &line[c + 7..];
        return Some(comdat_name(&r[..r.find(')').unwrap_or(r.len())]));
    }
    let i = line.find(" comdat")?;
    let after = &line[i + " comdat".len()..];
    after
        .chars()
        .next()
        .is_none_or(|c| c == ',' || c.is_whitespace())
        .then_some(name)
}

fn read(ir: &str) -> Module<'_> {
    let mut m = Module {
        header: Vec::new(),
        comments: Vec::new(),
        declarations: Vec::new(),
        comdats: HashMap::new(),
        items: Vec::new(),
        index: HashMap::new(),
    };
    let mut pos = 0;
    let bytes = ir.as_bytes();
    while pos < bytes.len() {
        let line_end = memchr_at(bytes, b'\n', pos).map_or(bytes.len(), |p| p + 1);
        let line = &ir[pos..line_end];
        if line.starts_with("define ") {
            // Up to the line that is just `}`.
            let mut end = line_end;
            loop {
                if end >= bytes.len() {
                    break;
                }
                let next = memchr_at(bytes, b'\n', end).map_or(bytes.len(), |p| p + 1);
                let l = &ir[end..next];
                end = next;
                if l.starts_with('}') {
                    break;
                }
            }
            let (at, _) = function_identifier(line).unwrap_or((0, 0));
            let name = name_in(&line[at + 1..]);
            let local = line[..at]
                .split_whitespace()
                .any(|word| word == "internal" || word == "private");
            let comdat = line_comdat(line, name);
            m.index.insert(name, m.items.len());
            m.items.push(Item {
                name,
                ty: ItemKind::Function,
                text: &ir[pos..end],
                local,
                mutable: false,
                comdat,
            });
            pos = end;
            continue;
        }
        if let Some(r) = line.strip_prefix('@') {
            let (name, width) = name_and_width(r);
            let after = &r[width..];
            let after = after
                .trim_start()
                .strip_prefix('=')
                .unwrap_or(after)
                .trim_start();
            let local = after.starts_with("private ") || after.starts_with("internal ");
            let mutable = local && global_head(after).0 == "global";
            let comdat = line_comdat(line, name);
            m.index.insert(name, m.items.len());
            m.items.push(Item {
                name,
                ty: ItemKind::Global,
                text: line,
                local,
                mutable,
                comdat,
            });
        } else if line.starts_with("declare ") {
            let name = function_identifier(line).map_or("", |(at, _)| name_in(&line[at + 1..]));
            m.declarations.push((name, line));
        } else if line.starts_with('$') {
            let n = line.split(' ').next().unwrap_or("");
            m.comdats.insert(comdat_name(n), line);
        } else if line.starts_with(';') {
            m.comments.push(line);
        } else {
            m.header.push(line);
        }
        pos = line_end;
    }
    m
}

/// `global`/`constant`, and the rest after the word, from `<linkage…>
/// global|constant <type> <initial>`.
fn global_head(s: &str) -> (&str, &str) {
    let mut rest = s;
    loop {
        let (word, after) = rest.split_once(' ').unwrap_or((rest, ""));
        if word == "global" || word == "constant" || word.is_empty() {
            return (word, after);
        }
        rest = after;
    }
}

/// The LLVM type at the start of `s` (with brackets, braces and `<…>` matched).
fn type_at_start(s: &str) -> &str {
    let b = s.as_bytes();
    let mut prof = 0i32;
    for (i, c) in b.iter().enumerate() {
        match c {
            b'[' | b'{' | b'<' | b'(' => prof += 1,
            b']' | b'}' | b'>' | b')' => {
                prof -= 1;
                if prof == 0 {
                    return &s[..=i];
                }
            }
            b' ' | b',' if prof == 0 => return &s[..i],
            _ => {}
        }
    }
    s
}

/// The declaration of an item defined in another part.
fn declaration(item: &Item) -> String {
    match item.ty {
        ItemKind::Global => {
            let width = name_and_width(&item.text[1..]).1;
            let identifier = &item.text[..1 + width];
            let r = &item.text[1 + width..];
            let r = r.trim_start().trim_start_matches('=').trim_start();
            // This already is a declaration. Preserve visibility, TLS models,
            // address spaces and exact escaped spelling (Mach-O's \01 prefix).
            if r.starts_with("external ") || r.starts_with("extern_weak ") {
                return item.text.to_string();
            }
            let (word, after) = global_head(r);
            let ty = type_at_start(after);
            // The thread-local variable (`@df.ctx.tls`, `sdk_module::with_context_in_tls`)
            // stays thread-local in the declaration.
            let tls = if r[..r.len() - after.len()]
                .split_whitespace()
                .any(|w| w == "thread_local")
            {
                "thread_local "
            } else {
                ""
            };
            format!("{identifier} = external {tls}{word} {ty}\n")
        }
        ItemKind::Function => {
            let line = item.text.lines().next().unwrap_or("");
            let (a, width) = function_identifier(line).unwrap_or((0, 0));
            let identifier = &line[a..a + 1 + width];
            let before = &line["define ".len()..a];
            let ret: Vec<&str> = before
                .split_whitespace()
                .filter(|w| {
                    !matches!(
                        *w,
                        "private"
                            | "internal"
                            | "linkonce_odr"
                            | "weak_odr"
                            | "external"
                            | "hidden"
                            | "dso_local"
                            | "dllexport"
                            | "fastcc"
                            | "ccc"
                    )
                })
                .collect();
            let after_identifier = a + 1 + width;
            let opens = after_identifier + line[after_identifier..].find('(').unwrap_or(0);
            // The parameters, with commas only at the top level.
            let mut prof = 0i32;
            let mut closes = opens;
            for (i, c) in line[opens..].char_indices() {
                match c {
                    '(' | '[' | '{' | '<' => prof += 1,
                    ')' | ']' | '}' | '>' => {
                        prof -= 1;
                        if prof == 0 {
                            closes = opens + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let params = &line[opens + 1..closes];
            let mut types = Vec::new();
            let mut prof = 0i32;
            let mut ini = 0;
            let mut parts = Vec::new();
            for (i, c) in params.char_indices() {
                match c {
                    '(' | '[' | '{' | '<' => prof += 1,
                    ')' | ']' | '}' | '>' => prof -= 1,
                    ',' if prof == 0 => {
                        parts.push(&params[ini..i]);
                        ini = i + 1;
                    }
                    _ => {}
                }
            }
            if !params.trim().is_empty() {
                parts.push(&params[ini..]);
            }
            for p in parts {
                let p = p.trim();
                let without_name = match p.rfind(" %") {
                    Some(k) => &p[..k],
                    None => p,
                };
                types.push(without_name.to_string());
            }
            format!(
                "declare {} {identifier}({})\n",
                ret.join(" "),
                types.join(", ")
            )
        }
    }
}

/// The grouping key of a symbol: its library, when the name
/// has it (`df.<library>.…`, `df.mt.<library>.…`, `df.rti.<h>.<library>`).
fn group(name: &str) -> &str {
    let parts: Vec<&str> = name.splitn(4, '.').collect();
    match parts.as_slice() {
        ["df", "mt", lib, ..] => lib,
        ["df", "rti", _, lib] => lib,
        ["df", lib, ..] => lib,
        _ => name,
    }
}

/// The split module: the parts are assembled one by one ([`Plan::part`]),
/// so that only those being generated exist at the same time.
pub struct Plan<'a> {
    m: Module<'a>,
    parts: Vec<Vec<usize>>,
    declared_set: HashMap<&'a str, &'a str>,
    target: usize,
}

impl<'a> Plan<'a> {
    /// Quantas partes.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// No part (never: a module has at least one).
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The text of part `p`.
    pub fn part(&self, p: usize) -> String {
        let m = &self.m;
        let target = self.target;
        let declared_set = &self.declared_set;
        let items = &self.parts[p];
        // The closure: the items of the part, the locals they cite (copied), and
        // the declarations of what lives outside.
        let mut included: HashSet<usize> = items.iter().copied().collect();
        let mut queue: Vec<usize> = items.to_vec();
        let mut externals: Vec<usize> = Vec::new();
        let mut seen_ext: HashSet<usize> = HashSet::new();
        let mut decls: Vec<&str> = Vec::new();
        let mut seen_decl: HashSet<&str> = HashSet::new();
        let mut refs = Vec::new();
        while let Some(i) = queue.pop() {
            refs.clear();
            references(m.items[i].text, &mut refs);
            for r in refs.iter() {
                if let Some(&j) = m.index.get(r) {
                    if j == i || included.contains(&j) {
                        continue;
                    }
                    let it = &m.items[j];
                    if it.local && !it.mutable {
                        included.insert(j);
                        queue.push(j);
                    } else if seen_ext.insert(j) {
                        externals.push(j);
                    }
                } else if let Some(d) = declared_set.get(r)
                    && seen_decl.insert(*r)
                {
                    decls.push(d);
                }
            }
        }
        let mut text = String::with_capacity(target + target / 4);
        for l in &m.header {
            text.push_str(l);
        }
        if p == 0 {
            for l in &m.comments {
                text.push_str(l);
            }
        }
        for d in &decls {
            text.push_str(d);
            if !d.ends_with('\n') {
                text.push('\n');
            }
        }
        externals.sort_unstable();
        for j in externals {
            text.push_str(&declaration(&m.items[j]));
        }
        let mut list: Vec<usize> = included.into_iter().collect();
        list.sort_unstable();
        for i in list {
            let it = &m.items[i];
            if let Some(c) = it.comdat
                && let Some(l) = m.comdats.get(c)
            {
                text.push_str(l);
            }
            if it.mutable {
                // `@df.area_id = internal global …` → external linkage.
                text.push_str(&it.text.replacen(" = internal ", " = ", 1).replacen(
                    " = private ",
                    " = ",
                    1,
                ));
            } else if !it.local && first_line(it.text).contains(" linkonce_odr ") {
                // `linkonce_odr` → `weak_odr`: the definition has to stay in the
                // object of its part. With ThinLTO (production) a
                // `linkonce_odr` that only other parts cited was discarded and
                // the link gave "undefined symbol: dfc.….get" (the
                // canonical constants of new_sali/backend).
                let first = first_line(it.text);
                text.push_str(&first.replacen(" linkonce_odr ", " weak_odr ", 1));
                text.push_str(&it.text[first.len()..]);
            } else {
                text.push_str(it.text);
            }
        }
        text
    }
}

/// Plans the split of the module into parts of ~`target` bytes.
pub fn make_plan(ir: &str, target: usize) -> Plan<'_> {
    let m = read(ir);
    // The items with external linkage, grouped by library in the order in
    // which they appear; the groups, in parts of ~`target` bytes.
    let mut group_order: Vec<&str> = Vec::new();
    let mut by_group: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, it) in m.items.iter().enumerate() {
        if it.local {
            continue;
        }
        let g = group(it.name);
        by_group
            .entry(g)
            .or_insert_with(|| {
                group_order.push(g);
                Vec::new()
            })
            .push(i);
    }
    let mut parts: Vec<Vec<usize>> = vec![Vec::new()];
    let mut size = 0usize;
    for g in &group_order {
        for &i in &by_group[g] {
            let t = m.items[i].text.len();
            if size > 0 && size + t > target {
                parts.push(Vec::new());
                size = 0;
            }
            parts.last_mut().expect("one part").push(i);
            size += t;
        }
    }
    // The local mutable state goes to part 0, with external linkage.
    for (i, it) in m.items.iter().enumerate() {
        if it.mutable {
            parts[0].push(i);
        }
    }
    let declared_set: HashMap<&str, &str> = m.declarations.iter().copied().collect();
    Plan {
        m,
        parts,
        declared_set,
        target,
    }
}

/// Splits the module into parts of ~`target` bytes (all at once; the tests).
/// A module smaller than `target` comes back whole, as it is.
pub fn split(ir: &str, target: usize) -> Vec<String> {
    if ir.len() <= target {
        return vec![ir.to_string()];
    }
    let plan = make_plan(ir, target);
    (0..plan.len()).map(|p| plan.part(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOD: &str = "target triple = \"x86_64-pc-windows-msvc\"\n\
; df.class 1 lib A\n\
declare i64 @dartforge_x(i64)\n\
@.str.0 = private unnamed_addr constant [2 x i8] c\"oi\"\n\
@df.area_id = internal global i64 0, align 8\n\
@df.a.g = global { i64, [2 x ptr] } zeroinitializer\n\
$\"df.b.k\" = comdat any\n\
define internal i64 @df.help(i64 %x) alwaysinline {\n\
  %a = load i64, ptr @df.area_id\n\
  ret i64 %x\n\
}\n\
define i64 @df.a.f(i64 %v0, { i64, i64 } %v1) {\n\
b0:\n\
  %r = call i64 @df.help(i64 %v0)\n\
  %s = call i64 @\"df.b.k\"(ptr @.str.0)\n\
  ret i64 %s\n\
}\n\
define linkonce_odr i64 @df.b.k(ptr %p) comdat($\"df.b.k\") {\n\
b0:\n\
  %q = call i64 @dartforge_x(i64 1)\n\
  %g = load i64, ptr @df.a.g\n\
  ret i64 %q\n\
}\n";

    #[test]
    fn splits_with_declarations_and_copies() {
        let p = split(MOD, 120);
        assert_eq!(p.len(), 3, "{p:#?}");
        // the part of `df.a.f` copies the helper and the string, declares `df.b.k`.
        let pa = p.iter().find(|t| t.contains("define i64 @df.a.f")).unwrap();
        assert!(pa.contains("define internal i64 @df.help"));
        assert!(pa.contains("@.str.0 = private"));
        assert!(pa.contains("declare i64 @df.b.k(ptr)"));
        assert!(
            pa.contains("@df.area_id = external global i64")
                || pa.contains("@df.area_id = global i64 0")
        );
        let pb = p
            .iter()
            .find(|t| t.contains("define weak_odr i64 @df.b.k"))
            .unwrap();
        assert!(pb.contains("$\"df.b.k\" = comdat any"));
        assert!(pb.contains("declare i64 @dartforge_x(i64)"));
        assert!(
            pb.contains("@df.a.g = external global { i64, [2 x ptr] }"),
            "{pb}"
        );
        // The mutable state: defined once, with external linkage.
        assert_eq!(
            p.iter()
                .filter(|t| t.contains("@df.area_id = global i64 0"))
                .count(),
            1
        );
        assert!(p[0].contains("; df.class"));
    }

    /// The thread-local variable of the context (`sdk_module::with_context_in_tls`)
    /// is state: one definition, and the declaration in the other parts stays
    /// `thread_local`.
    #[test]
    fn thread_local_variable_stays_thread_local() {
        let m = read("@df.ctx.tls.program = internal thread_local global ptr null, align 8\n");
        assert!(m.items[0].mutable);
        assert_eq!(
            declaration(&m.items[0]),
            "@df.ctx.tls.program = external thread_local global ptr\n"
        );
    }

    #[test]
    fn declares_function_with_composite_parameters() {
        let m = read(MOD);
        let i = m.index["df.a.f"];
        assert_eq!(
            declaration(&m.items[i]),
            "declare i64 @df.a.f(i64, { i64, i64 })\n"
        );
    }

    #[test]
    fn implicit_comdat_goes_with_item() {
        let m = "$df.img.a = comdat any
@df.img.a = linkonce_odr constant i64 1, comdat
define i64 @df.x.f() {
b0:
  %v = load i64, ptr @df.img.a
  ret i64 %v
}
";
        let p = split(m, 40);
        let def = p
            .iter()
            .find(|t| t.contains("@df.img.a = weak_odr"))
            .unwrap();
        assert!(def.contains("$df.img.a = comdat any"), "{def}");
        let uses = p.iter().find(|t| t.contains("define i64 @df.x.f")).unwrap();
        assert!(
            uses.contains("@df.img.a = external constant i64")
                || uses.contains("@df.img.a = weak_odr"),
            "{uses}"
        );
    }
    #[test]
    fn macho_external_section_identifiers_keep_exact_escapes_and_visibility() {
        let ir = r#"@"\01section$start$__DATA$__dfpcl" = external hidden global i8
@"\01section$end$__DATA$__dfpcl" = external hidden global i8
define ptr @reader() {
  ret ptr @"\01section$start$__DATA$__dfpcl"
}
"#;
        let parts = split(ir, 1);
        let reader = parts
            .iter()
            .find(|part| part.contains("define ptr @reader"))
            .unwrap();
        assert!(
            reader.contains(r#"@"\01section$start$__DATA$__dfpcl" = external hidden global i8"#),
            "{reader}"
        );
        assert!(!reader.contains(r#"@\01section"#), "{reader}");
        let module = read(ir);
        assert_eq!(declaration(&module.items[0]), module.items[0].text);
    }

    #[test]
    fn quoted_definition_names_survive_cross_part_declarations() {
        let ir = r#"@"state \22quoted\5C" = internal thread_local global i64 0
define i64 @"function(@ odd \22name\5C"(i64 %value) {
  %state = load i64, ptr @"state \22quoted\5C"
  ret i64 %value
}
define i64 @caller() {
  %state = load i64, ptr @"state \22quoted\5C"
  %value = call i64 @"function(@ odd \22name\5C"(i64 42)
  ret i64 %value
}
"#;
        let parts = split(ir, 1);
        let caller = parts
            .iter()
            .find(|part| part.contains("define i64 @caller"))
            .unwrap();
        assert!(
            caller.contains(r#"declare i64 @"function(@ odd \22name\5C"(i64)"#),
            "{caller}"
        );
        assert!(
            caller.contains(r#"@"state \22quoted\5C" = external thread_local global i64"#),
            "{caller}"
        );
        assert_eq!(parts.iter().filter(|part| part.contains(r#"@"state \22quoted\5C" = thread_local global i64 0"#)).count(), 1);
    }
    #[test]
    fn clang_accepts_every_split_part_with_quoted_macho_and_function_identifiers() {
        let ir = r#"@"\01section$start$__DATA$__dfpcl" = external hidden global i8
@"quoted\22name\5C" = global i64 1
define ptr @"pointer(@ function"() {
  ret ptr @"\01section$start$__DATA$__dfpcl"
}
define i64 @caller() {
  %value = load i64, ptr @"quoted\22name\5C"
  %pointer = call ptr @"pointer(@ function"()
  ret i64 %value
}
"#;
        let directory = tempfile::tempdir().unwrap();
        let clang = crate::driver::NativeDriverOptions::default().clang;
        let parts = split(ir, 1);
        assert!(parts.len() > 1);
        for (index, part) in parts.iter().enumerate() {
            let input = directory.path().join(format!("part{index}.ll"));
            let output = directory.path().join(format!("part{index}.bc"));
            std::fs::write(&input, part).unwrap();
            let result = std::process::Command::new(&clang)
                .args(["-x", "ir", "-c", "-emit-llvm"])
                .arg(&input)
                .arg("-o")
                .arg(&output)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "part {index}: {}\n{part}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(output.is_file());
        }
    }
}
