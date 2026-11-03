//! The class names and subtype edges of the SDK libraries of a WebAssembly
//! module, registered from data with only the classes the live code can meet.
//!
//! Each SDK library registered at startup the name of every instantiable class
//! and every direct subtype edge it declares (a hello world: 457 names and
//! 1 697 edges, ~28 KB of straight-line calls). On WebAssembly the library
//! writes them as comments instead (`; df.class-name <cid> <name>` and the
//! `; df.cone-parent` lines its header already has), the summary carries them
//! ([`super::Summary::class_names`], [`super::Summary::parent_edges`]), and the
//! link writes one byte string with those of the classes the RTI table keeps
//! ([`super::rti_table::kept`]) — the classes of the objects the live code
//! allocates, the runtime's own and the ones a live type cites — closed over
//! the edges. `@df.register.sdk_classes`, which the program calls in
//! `df.prepare_isolate`, hands it to the runtime
//! (`dartforge_register_classes`).
//!
//! The runtime reads a class name only for one of its instances, and asks the
//! subtype graph about the class of a value or of a type, so a class outside
//! the kept set is never asked about, and neither is a supertype the edges
//! from the kept ones do not reach.

use super::Summary;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// The function the program declares and the link defines.
pub const REGISTRATION: &str = "df.register.sdk_classes";

/// Whether `name` can travel in a `; df.class-name` comment (and a summary
/// line): a class name without control characters. Any other name keeps its
/// startup call.
///
/// ```
/// use dartforge_emit_native::pruning::class_registry::writable_name;
/// assert!(writable_name("_GrowableList"));
/// assert!(writable_name("_A&B"));
/// assert!(!writable_name(""));
/// assert!(!writable_name("a\tb"));
/// ```
pub fn writable_name(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(char::is_control)
}

/// The class name of a `; df.class-name <cid> <name>` line.
pub(super) fn read_comment(line: &str) -> Option<(i64, String)> {
    let rest = line.strip_prefix("; df.class-name ")?;
    let (cid, name) = rest.split_once(' ')?;
    let cid = cid.parse::<u32>().ok()?;
    writable_name(name).then(|| (i64::from(cid), name.to_string()))
}

fn leb(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

/// The data of `dartforge_register_classes` (`runtime/src/core.rs`) for the
/// SDK classes of `sdk` in `kept` closed over the SDK edges (every class
/// without `kept`): unsigned LEB128 numbers, the count of names, each name as
/// its class id, length and bytes, then each edge as subclass and superclass.
///
/// ```
/// use dartforge_emit_native::pruning::{class_registry::data, Summary};
/// use std::collections::HashSet;
/// let mut s = Summary::default();
/// s.class_names = vec![(10, "A".into()), (11, "B".into()), (12, "C".into())];
/// s.parent_edges = vec![(10, 0), (10, 13), (13, 0), (11, 0)];
/// let kept: HashSet<i64> = [10].into_iter().collect();
/// // A's name, its edges and those of its supertype 13 (which has no name).
/// assert_eq!(data(&[&s], Some(&kept)), vec![1, 10, 1, b'A', 10, 0, 10, 13, 13, 0]);
/// ```
pub fn data(sdk: &[&Summary], kept: Option<&HashSet<i64>>) -> Vec<u8> {
    let mut keep: HashSet<i64> = HashSet::new();
    if let Some(kept) = kept {
        let mut parents: HashMap<i64, Vec<i64>> = HashMap::new();
        for s in sdk {
            for &(child, parent) in &s.parent_edges {
                parents.entry(child).or_default().push(parent);
            }
        }
        let mut queue: Vec<i64> = kept.iter().copied().collect();
        while let Some(c) = queue.pop() {
            if keep.insert(c)
                && let Some(more) = parents.get(&c)
            {
                queue.extend_from_slice(more);
            }
        }
    }
    let wanted = |cid: i64| kept.is_none() || keep.contains(&cid);
    let mut names: Vec<(i64, &str)> = sdk
        .iter()
        .flat_map(|s| s.class_names.iter())
        .filter(|(cid, _)| wanted(*cid))
        .map(|(cid, name)| (*cid, name.as_str()))
        .collect();
    names.sort_unstable();
    names.dedup();
    let mut edges: Vec<(i64, i64)> = sdk
        .iter()
        .flat_map(|s| s.parent_edges.iter().copied())
        .filter(|(child, _)| wanted(*child))
        .collect();
    edges.sort_unstable();
    edges.dedup();
    let mut out = Vec::new();
    leb(&mut out, names.len() as u64);
    for (cid, name) in names {
        leb(&mut out, cid as u64);
        leb(&mut out, name.len() as u64);
        out.extend_from_slice(name.as_bytes());
    }
    for (child, parent) in edges {
        leb(&mut out, child as u64);
        leb(&mut out, parent as u64);
    }
    out
}

/// The definition of [`REGISTRATION`] with the [`data`] of `sdk`, appended to
/// the assembled IR `out`.
pub(super) fn emit(
    out: &mut String,
    sdk: &[&Summary],
    kept: Option<&HashSet<i64>>,
    declared: &mut HashSet<String>,
) {
    let bytes = data(sdk, kept);
    let mut text = String::with_capacity(bytes.len() * 3);
    for b in &bytes {
        if b.is_ascii_alphanumeric() || *b == b' ' || *b == b'_' {
            text.push(*b as char);
        } else {
            write!(text, "\\{b:02X}").unwrap();
        }
    }
    writeln!(
        out,
        "@df.sdk_classes = private unnamed_addr constant [{} x i8] c\"{text}\"",
        bytes.len()
    )
    .unwrap();
    writeln!(
        out,
        "define void @{REGISTRATION}() {{\nb0:\n  call void @dartforge_register_classes(ptr @df.sdk_classes, i64 {})\n  ret void\n}}",
        bytes.len()
    )
    .unwrap();
    if declared.insert("dartforge_register_classes".into()) {
        out.push_str("declare void @dartforge_register_classes(ptr, i64)\n");
    }
}
