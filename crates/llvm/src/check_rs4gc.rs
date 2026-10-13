//! The root checker run **after** `rewrite-statepoints-for-gc`
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4, in the style of Perry's
//! `--statepoints`), on the §14.12 form: a `Ref` value is an `i64` (`%v<n>`)
//! throughout the code, and the roots live across a collecting call go in its
//! `"deopt"` operand — the statepoint carries them to the stack map.
//!
//! A collection only sees what is in the `"deopt"` of the statepoint where it
//! happens. The defect this checker catches is a value used after a statepoint
//! without being in its `"deopt"`: the object may have been freed (the optimizer
//! stretching the life of a `Ref` over a call, the emitter forgetting the
//! operand). Arguments too: a `Ref` passed to the statepoint's call must be live
//! in it (contract C1).
//!
//! A name is a `Ref` when it appears in the `"deopt"` of some statepoint of the
//! function (the `i64` does not say whether it is a reference). Excluded are the
//! value of an `!invariant.load` load, or of the context's canonical `true`/`false`
//! objects (offsets 360 and 368, `layout::context`; the optimization that merges
//! the loads drops the metadata), and a `phi` of only those, like a loop's
//! `.lcssa`: the canonical objects are statics of the runtime image, never
//! collected, and the optimizer reuses the load across calls, as it should. The
//! check is per block, on the module text: a use after a statepoint of the same
//! block, of a value that was not defined after it. A value that crosses the
//! statepoint into a `phi` of another block is not checked.

use std::collections::HashSet;

/// The `%x` names used in `text` (without the `%`).
fn uses_list(text: &str) -> impl Iterator<Item = &str> {
    let b = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < b.len() {
            if b[i] == b'%' {
                let begin = i + 1;
                let mut end = begin;
                while end < b.len() && (b[end].is_ascii_alphanumeric() || matches!(b[end], b'.' | b'_' | b'$' | b'-')) {
                    end += 1;
                }
                i = end.max(begin);
                if end > begin {
                    return Some(&text[begin..end]);
                }
            } else {
                i += 1;
            }
        }
        None
    })
}

/// The content of a line's `"deopt"(…)` operand, if it has one.
fn deopt_operand(line: &str) -> Option<&str> {
    let k = line.find("\"deopt\"(")?;
    let rest = &line[k + "\"deopt\"(".len()..];
    let mut depth = 1;
    for (i, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&rest[..i]);
                }
            }
            _ => {}
        }
    }
    Some(rest)
}

/// The line without the operand bundles (`[ … ]` after the arguments).
fn without_operands(body: &str) -> &str {
    body.find(" [ \"").map_or(body, |p| &body[..p])
}

/// The name defined by the line (`%x = …`), without the `%`.
fn defined(line: &str) -> Option<&str> {
    let l = line.trim_start().strip_prefix('%')?;
    let end = l.find(" = ")?;
    Some(&l[..end])
}

/// `refs` without the `!invariant.load` load values and without the `phi`s whose
/// inputs are all of those (or constants).
fn without_invariants<'a>(line_list: &[&'a str], mut refs: HashSet<&'a str>) -> HashSet<&'a str> {
    // The addresses of the canonical objects: `getelementptr … i64 360|368`,
    // directly or through the offset chosen in a `select` (`df.box_bool`).
    let selections: HashSet<&str> = line_list
        .iter()
        .filter(|l| l.contains("= select ") && l.contains("i64 360") && l.contains("i64 368"))
        .filter_map(|l| defined(l))
        .collect();
    let canonicals: HashSet<&str> = line_list
        .iter()
        .filter(|l| {
            l.contains("= getelementptr ")
                && (l.ends_with("i64 360") || l.ends_with("i64 368") || selections.iter().any(|s| l.ends_with(&format!("i64 %{s}"))))
        })
        .filter_map(|l| defined(l))
        .collect();
    let mut invariants: HashSet<&str> = line_list
        .iter()
        .filter(|l| {
            l.contains("!invariant.load")
                || l.contains("= load i64, ptr %")
                    && uses_list(&l[l.find("= load").unwrap_or(0)..]).next().is_some_and(|p| canonicals.contains(p))
        })
        .filter_map(|l| defined(l))
        .collect();
    // The `phi`s (with each one's block), the `icmp eq`s and the conditional
    // branches of each block: a `phi` input that arrives through the edge where
    // `X == canonical` is the canonical object itself (the optimizer swaps one for
    // the other).
    let mut phis: Vec<(&str, &str, Vec<(&str, &str)>)> = Vec::new();
    let mut equal: std::collections::HashMap<&str, (&str, &str)> = std::collections::HashMap::new();
    let mut branches: std::collections::HashMap<&str, (&str, &str)> = std::collections::HashMap::new();
    let mut block = "";
    for l in line_list {
        if !l.starts_with(' ') && !l.starts_with("define") && let Some(k) = l.find(':') {
            block = &l[..k];
            continue;
        }
        let t = l.trim_start();
        if let Some(rest) = t.strip_prefix("br i1 %") {
            // `br i1 %c, label %v, label %f`
            let mut part_list = rest.split(", label %");
            if let (Some(c), Some(v)) = (part_list.next(), part_list.next()) {
                branches.insert(block, (c, v.trim()));
            }
            continue;
        }
        let Some(d) = defined(t) else { continue };
        let body = &t[t.find(" = ").map_or(0, |k| k + 3)..];
        if body.starts_with("icmp eq i64 ") {
            let mut u = uses_list(body);
            if let (Some(a), Some(b)) = (u.next(), u.next()) {
                equal.insert(d, (a, b));
            }
        } else if body.starts_with("phi ") {
            let entries = body
                .split('[')
                .skip(1)
                .filter_map(|pair| {
                    let mut x = pair.split(',');
                    let value = x.next()?.trim().strip_prefix('%')?;
                    let of = x.next()?.trim().trim_end_matches(']').trim().strip_prefix('%')?;
                    Some((value, of))
                })
                .collect();
            phis.push((d, block, entries));
        }
    }
    loop {
        let before = invariants.len();
        for (d, block, entries) in &phis {
            if invariants.contains(d) || entries.is_empty() {
                continue;
            }
            let canonical_one = |value: &str, of: &str| {
                invariants.contains(value)
                    || branches.get(of).is_some_and(|(c, v)| {
                        v == block
                            && equal.get(c).is_some_and(|(a, b)| (*a == value && invariants.contains(b)) || (*b == value && invariants.contains(a)))
                    })
            };
            if entries.iter().all(|(value, of)| canonical_one(value, of)) {
                invariants.insert(d);
            }
        }
        if invariants.len() == before {
            break;
        }
    }
    refs.retain(|r| !invariants.contains(r));
    refs
}

/// A statepoint already seen in the block: the roots live in it and the names
/// defined after it.
struct Point<'a> {
    live_set: HashSet<&'a str>,
    after: HashSet<&'a str>,
    line: usize,
}

/// Checks the IR of a module after RS4GC.
///
/// # Errors
/// The first violation: the line of the use, the value and the statepoint's line.
pub fn check(ir: &str) -> Result<(), String> {
    let line_list: Vec<&str> = ir.lines().collect();
    let mut i = 0;
    while i < line_list.len() {
        if !line_list[i].starts_with("define ") {
            i += 1;
            continue;
        }
        let begin = i;
        let mut end = i + 1;
        while end < line_list.len() && !line_list[end].starts_with('}') {
            end += 1;
        }
        check_function(&line_list[begin..end.min(line_list.len())], begin)?;
        i = end + 1;
    }
    Ok(())
}

fn check_function(line_list: &[&str], offset: usize) -> Result<(), String> {
    // The function's `Ref`s: the names in some statepoint's `"deopt"`.
    let refs: HashSet<&str> = line_list
        .iter()
        .filter(|l| l.contains("@llvm.experimental.gc.statepoint"))
        .filter_map(|l| deopt_operand(l))
        .flat_map(uses_list)
        .collect();
    if refs.is_empty() {
        return Ok(());
    }
    let refs = without_invariants(line_list, refs);
    let mut points: Vec<Point<'_>> = Vec::new();
    for (k, l) in line_list.iter().enumerate().skip(1) {
        let n = offset + k + 1;
        let t = l.trim_start();
        // A label starts a new block; comments and empty lines do not count.
        if !l.starts_with(' ') && !t.is_empty() {
            points.clear();
            continue;
        }
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        let def = defined(t);
        let body = match t.find(" = ") {
            Some(p) if def.is_some() => &t[p + 3..],
            _ => t,
        };
        let is_statepoint = body.contains("@llvm.experimental.gc.statepoint");
        // The uses on this line after the block's previous statepoints.
        if !body.contains("@llvm.experimental.gc.result") {
            for name in uses_list(without_operands(body)).filter(|x| refs.contains(x)) {
                for p in &points {
                    if !p.after.contains(name) && !p.live_set.contains(name) {
                        let sp = line_list[p.line - offset - 1].trim_start();
                        return Err(format!(
                            "RS4GC checker, line {n}: `%{name}` used after the statepoint on line {} without being in its \"deopt\": `{t}`\n  {sp}",
                            p.line
                        ));
                    }
                }
            }
        }
        if is_statepoint {
            let live_set: HashSet<&str> = deopt_operand(body).map(|d| uses_list(d).collect()).unwrap_or_default();
            // C1: the `Ref` argument is live in the call.
            for name in uses_list(without_operands(body)).filter(|x| refs.contains(x)) {
                if !live_set.contains(name) {
                    return Err(format!(
                        "RS4GC checker, line {n}: the argument `%{name}` is not live in the call (outside the \"deopt\"): `{t}`"
                    ));
                }
            }
            points.push(Point { live_set, after: HashSet::new(), line: n });
        }
        if let Some(d) = def {
            for p in &mut points {
                p.after.insert(d);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    // Raw strings: the indentation of the instructions is what separates an
    // instruction from a label.
    const BOM: &str = r#"define i64 @f(i64 %v1) gc "statepoint-example" {
b0:
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 (i64)) @g, i32 1, i32 0, i64 %v1, i32 0, i32 0) [ "deopt"(i64 %v1) ]
  %v2 = call i64 @llvm.experimental.gc.result.i64(token %sp)
  %v3 = add i64 %v1, %v2
  ret i64 %v3
}
"#;

    #[test]
    fn accepts_live_root_at_statepoint() {
        assert_eq!(check(BOM), Ok(()));
    }

    /// The planted violations: each one must be found. `%v1` stays a `Ref` through
    /// the `"deopt"` of a second statepoint.
    #[test]
    fn finds_planted_violations() {
        let other_one = "  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ \"deopt\"(i64 %v1) ]\n  ret i64 %v3";
        let without_root = BOM.replace(r#"[ "deopt"(i64 %v1) ]"#, r#"[ "deopt"() ]"#).replace("  ret i64 %v3", other_one);
        assert_ne!(without_root, BOM);
        // The later use, without the value among the arguments.
        let only_after = without_root.replace("i32 1, i32 0, i64 %v1, i32 0, i32 0)", "i32 0, i32 0, i32 0, i32 0)");
        assert!(check(&only_after).unwrap_err().contains("used after"), "{only_after}");
        // The argument without a root.
        assert!(check(&without_root).unwrap_err().contains("argument"), "{without_root}");
    }

    /// A value defined after the statepoint does not need to be in it; a new label
    /// resets the block's statepoints.
    #[test]
    fn does_not_flag_defined_after_or_other_block() {
        let ir = r#"define i64 @f(i64 %v1) gc "statepoint-example" {
b0:
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 ()) @g, i32 0, i32 0, i32 0, i32 0) [ "deopt"() ]
  %v4 = call i64 @llvm.experimental.gc.result.i64(token %sp)
  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 ()) @g, i32 0, i32 0, i32 0, i32 0) [ "deopt"(i64 %v4, i64 %v1) ]
  br label %b1
b1:
  %v5 = add i64 %v4, %v1
  ret i64 %v5
}
"#;
        assert_eq!(check(ir), Ok(()));
    }

    /// An invariant load (the context's canonical `true`) reused across a
    /// statepoint is not flagged.
    #[test]
    fn does_not_flag_invariant_load() {
        let ir = r#"define i64 @f(ptr %ctx) gc "statepoint-example" {
b0:
  %vh = load i64, ptr %ctx, align 8, !invariant.load !0
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(i64 (i64)) @g, i32 1, i32 0, i64 %vh, i32 0, i32 0) [ "deopt"(i64 %vh) ]
  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ "deopt"() ]
  %v2 = add i64 %vh, 1
  ret i64 %v2
}
"#;
        assert_eq!(check(ir), Ok(()));
    }

    /// The `phi` that joins the canonical object with the value the edge compares to it.
    #[test]
    fn does_not_flag_phi_guarded_by_equality() {
        let ir = r#"define i64 @f(ptr %ctx, i64 %v1) gc "statepoint-example" {
b0:
  %fp = getelementptr inbounds nuw i8, ptr %ctx, i64 368
  %fh = load i64, ptr %fp, align 8
  %c = icmp eq i64 %v1, %fh
  br i1 %c, label %b2, label %b1
b1:
  br label %b2
b2:
  %x = phi i64 [ %v1, %b0 ], [ %fh, %b1 ]
  %sp = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ "deopt"(i64 %v1) ]
  %sp2 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void (i64)) @g, i32 1, i32 0, i64 %x, i32 0, i32 0) [ "deopt"(i64 %x) ]
  %sp3 = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr elementtype(void ()) @h, i32 0, i32 0, i32 0, i32 0) [ "deopt"() ]
  ret i64 %x
}
"#;
        assert_eq!(check(ir), Ok(()));
        // Without the guard, the `phi` is a reference and the later use is flagged.
        let without = ir.replace("br i1 %c, label %b2, label %b1", "br label %b2");
        assert!(check(&without).is_err());
    }

    #[test]
    fn reads_names_and_operand() {
        assert_eq!(uses_list("add i64 %v1, %v2.i").collect::<Vec<_>>(), ["v1", "v2.i"]);
        assert_eq!(defined("  %v3 = add i64 1, 2"), Some("v3"));
        assert_eq!(deopt_operand("call void @g() [ \"deopt\"(i64 %v1, i64 %v2) ]"), Some("i64 %v1, i64 %v2"));
        assert_eq!(without_operands("call void @g(i64 %v3) [ \"deopt\"(i64 %v1) ]"), "call void @g(i64 %v3)");
    }
}
