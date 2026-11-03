//! The RTI table of a WebAssembly module with only the classes the live code
//! can meet (a hello world registered 645 classes: 72 KB of text and, being
//! above the compression threshold, the decompressor).
//!
//! The lowering writes the whole table (`lower::rti::register_universe`), as
//! every target reads it, and the emitter repeats it by class in comments of
//! the program IR ([`comments`]). The link assembly ([`super::assemble`])
//! reads them back and, with the fixpoint of the link, writes the table again
//! ([`rewrite`]). It keeps:
//!
//! * the table's roots ([`RtiTable::roots`]: the runtime shapes, its error
//!   classes, `Object` and the fixed cids, whose values the runtime makes
//!   without an allocation of the program);
//! * every class with a live method table: the classes of the objects the code
//!   allocates;
//! * every class a live text cites (`C<id>`, [`crate::lower::rti::class_citations`]):
//!   the types the code tests, casts, stores and prints;
//! * closed over them, the supertypes of each kept class and the classes of
//!   their type arguments ([`RtiClass::cites`]).
//!
//! The type of a value, the type of a recipe and every supertype reached from
//! either is then in the table, which is all the runtime asks of it
//! (`runtime/src/types.rs`, `dartforge_rti_init_table`). The `B` lines (the
//! library of each class) stay only when the `dart:mirrors` natives that read
//! them are live, and a table below `lower::rti::WASM_COMPRESSION_THRESHOLD`
//! goes uncompressed, without the decompressor.

use super::{Reach, Summary};
use crate::hir::{RtiClass, RtiTable};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// The global of the rewritten table.
const GLOBAL: &str = "df.rti.table";

/// The comment lines of `table` (the IR of a WebAssembly program).
///
/// ```
/// use dartforge_emit_native::hir::{RtiClass, RtiTable};
/// use dartforge_emit_native::pruning::rti_table::{comments, read};
/// let t = RtiTable {
///     head: "F0 2\n".into(),
///     classes: vec![RtiClass { id: 2, lines: "C2 0 int\nR2 C0\n".into(), cites: vec![0] }],
///     tail: String::new(),
///     roots: vec![0, 2],
/// };
/// let text = comments(&t);
/// let back = read(&text).unwrap();
/// assert_eq!((back.head, back.roots), (t.head, t.roots));
/// assert_eq!((back.classes[0].id, &back.classes[0].lines), (2, &t.classes[0].lines));
/// ```
pub fn comments(table: &RtiTable) -> String {
    let mut s = String::new();
    s.push_str("; df.rti-root");
    for r in &table.roots {
        write!(s, " {r}").unwrap();
    }
    s.push('\n');
    for l in table.head.lines() {
        writeln!(s, "; df.rti-head {l}").unwrap();
    }
    for c in &table.classes {
        write!(s, "; df.rti-class {}", c.id).unwrap();
        for x in &c.cites {
            write!(s, " {x}").unwrap();
        }
        s.push('\n');
        for l in c.lines.lines() {
            writeln!(s, "; df.rti-line {l}").unwrap();
        }
    }
    for l in table.tail.lines() {
        writeln!(s, "; df.rti-tail {l}").unwrap();
    }
    s
}

/// The table of the [`comments`] in `ir`; `None` without them.
pub fn read(ir: &str) -> Option<RtiTable> {
    let mut table: Option<RtiTable> = None;
    for l in ir.lines() {
        let Some(rest) = l.strip_prefix("; df.rti-") else {
            continue;
        };
        let t = table.get_or_insert_with(RtiTable::default);
        let numbers = |s: &str| -> Vec<i64> {
            s.split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect()
        };
        if let Some(r) = rest.strip_prefix("root") {
            t.roots = numbers(r);
        } else if let Some(h) = rest.strip_prefix("head ") {
            t.head.push_str(h);
            t.head.push('\n');
        } else if let Some(c) = rest.strip_prefix("class ") {
            let v = numbers(c);
            t.classes.push(RtiClass {
                id: v.first().copied()?,
                lines: String::new(),
                cites: v[1..].to_vec(),
            });
        } else if let Some(x) = rest.strip_prefix("line ") {
            let c = t.classes.last_mut()?;
            c.lines.push_str(x);
            c.lines.push('\n');
        } else if let Some(x) = rest.strip_prefix("tail ") {
            t.tail.push_str(x);
            t.tail.push('\n');
        }
    }
    table
}

/// The classes of `table` that the live code of `reach` can meet (see the
/// module documentation). `DARTFORGE_TRACE_RTI=1` writes why each one enters
/// on stderr.
pub fn kept(table: &RtiTable, summaries: &[&Summary], reach: &Reach) -> HashSet<i64> {
    let trace = std::env::var("DARTFORGE_TRACE_RTI").is_ok_and(|v| v == "1");
    let index: HashMap<&str, u32> = reach
        .names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();
    let live = |n: &str| index.get(n).is_some_and(|&i| reach.live(i));
    let mut queue: Vec<i64> = table.roots.clone();
    for r in summaries {
        let name = |node: u32| r.names[node as usize].as_str();
        for t in &r.tables {
            if live(name(t.node)) {
                if trace {
                    eprintln!("[rti] C{}: live table {}", t.cid, name(t.node));
                }
                queue.push(t.cid);
            }
        }
        for (node, text) in &r.recipes {
            if live(name(*node)) {
                if trace {
                    eprintln!("[rti] {text}: live text {}", name(*node));
                }
                queue.extend(crate::lower::rti::class_citations(text));
            }
        }
    }
    let cites: HashMap<i64, &[i64]> = table
        .classes
        .iter()
        .map(|c| (c.id, c.cites.as_slice()))
        .collect();
    let mut keep: HashSet<i64> = HashSet::new();
    while let Some(c) = queue.pop() {
        if keep.insert(c)
            && let Some(more) = cites.get(&c)
        {
            if trace && !more.is_empty() {
                eprintln!("[rti] {more:?}: cited by the rules of C{c}");
            }
            queue.extend_from_slice(more);
        }
    }
    if trace {
        eprintln!(
            "[rti] {} of {} classes kept",
            table
                .classes
                .iter()
                .filter(|c| keep.contains(&c.id))
                .count(),
            table.classes.len()
        );
    }
    keep
}

/// The rewritten table of an IR with [`comments`]: the runtime entry and the
/// length of the new call, and the global with the bytes (to append to the
/// IR).
pub struct Rewritten {
    pub entry: &'static str,
    pub len: usize,
    pub global: String,
}

impl Rewritten {
    /// `line`, the lowering's call of the table entry ([`is_table_call`]),
    /// calling [`Self::entry`] with the new global and length; the rest of the
    /// line (the result, the attributes) stays.
    ///
    /// ```
    /// use dartforge_emit_native::pruning::rti_table::Rewritten;
    /// let r = Rewritten { entry: "dartforge_rti_init_table", len: 9, global: String::new() };
    /// assert_eq!(
    ///     r.call("  call void @dartforge_rti_init_table_compressed(ptr @df.arr.3, i64 400) #2\n"),
    ///     "  call void @dartforge_rti_init_table(ptr @df.rti.table, i64 9) #2\n"
    /// );
    /// ```
    pub fn call(&self, line: &str) -> String {
        let Some(start) = line.find("@dartforge_rti_init_table") else {
            return line.to_string();
        };
        let end = line[start..]
            .find(')')
            .map_or(line.len(), |e| start + e + 1);
        format!(
            "{}@{}(ptr @{GLOBAL}, i64 {}){}",
            &line[..start],
            self.entry,
            self.len,
            &line[end..]
        )
    }
}

/// The table of `ir` with only the [`kept`] classes; `None` when the IR has no
/// table comments.
pub fn rewrite(ir: &str, summaries: &[&Summary], reach: &Reach) -> Option<Rewritten> {
    let table = read(ir)?;
    let keep = kept(&table, summaries, reach);
    // The `B` lines (library and declared name of each class) only feed the
    // `dart:mirrors` subset (`runtime/src/compat_jit.rs`).
    let libraries =
        reach.names.iter().enumerate().any(|(i, n)| {
            n.starts_with("dartforge_native_DartForge_mirrors_") && reach.live(i as u32)
        });
    let (bytes, compressed) = crate::lower::rti::table_bytes(&table, Some(&keep), libraries);
    let words = crate::lower::rti::table_words(&bytes);
    let entry = crate::lower::rti::table_entry(compressed);
    let mut global = format!(
        "@{GLOBAL} = private unnamed_addr constant [{} x i64] [",
        words.len()
    );
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            global.push_str(", ");
        }
        write!(global, "i64 {w}").unwrap();
    }
    global.push_str("], align 8\n");
    Some(Rewritten {
        entry,
        len: bytes.len(),
        global,
    })
}

/// Whether `line` is the lowering's call of the table entry (a `call`, or an
/// `invoke` under table exceptions; not its declaration).
pub fn is_table_call(line: &str) -> bool {
    !line.starts_with("declare ") && line.contains("@dartforge_rti_init_table")
}
