//! Physical ARC dispatch guards; object inventory and logical tokens remain intact.

use std::fmt::Write;

pub(super) const RETAIN: &str = "df.arc_retain_ref";
pub(super) const RELEASE: &str = "df.arc_release_ref";

pub(super) const HELPERS: &str = r#"
define internal void @df.arc_retain_ref(i64 %value) alwaysinline nounwind "gc-leaf-function" {
entry:
  %nonnull = icmp ne i64 %value, 0
  %tag = and i64 %value, 1
  %even = icmp eq i64 %tag, 0
  %handle = and i1 %nonnull, %even
  br i1 %handle, label %object, label %done
object:
  call void @dartforge_arc_retain(i64 %value)
  br label %done
done:
  ret void
}
define internal void @df.arc_release_ref(i64 %value) alwaysinline nounwind "gc-leaf-function" {
entry:
  %nonnull = icmp ne i64 %value, 0
  %tag = and i64 %value, 1
  %even = icmp eq i64 %tag, 0
  %handle = and i1 %nonnull, %even
  br i1 %handle, label %object, label %done
object:
  call void @dartforge_arc_release(i64 %value)
  br label %done
done:
  ret void
}
"#;

/// The block of a live object of the space in `%value` and its in-object word `%w`
/// at `%wp`; to `%none` for a scalar or a static, to `%slow` without the published
/// size.
fn prompt_word(none: &str, slow: &str) -> String {
    use dartforge_runtime::layout::{self, arc_word};
    {
        format!(
            r#"  %nonnull = icmp ne i64 %value, 0
  %tag = and i64 %value, 1
  %even = icmp eq i64 %tag, 0
  %handle = and i1 %nonnull, %even
  br i1 %handle, label %object, label %{none}
object:
  %b = add i64 %value, -{handle}
  %bp = inttoptr i64 %b to ptr
  %st = load i8, ptr %bp, align 1
  %static = icmp eq i8 %st, {permanent}
  br i1 %static, label %{none}, label %page
page:
  %pg = and i64 %b, -{page}
  %pgp = inttoptr i64 %pg to ptr
  %sp = getelementptr inbounds i8, ptr %pgp, i64 {head}
  %size = load i64, ptr %sp, align 8
  %unpublished = icmp eq i64 %size, 0
  br i1 %unpublished, label %{slow}, label %word
word:
  %end = add i64 %b, %size
  %wa = add i64 %end, -8
  %wp = inttoptr i64 %wa to ptr
  %w = load i64, ptr %wp, align 8
"#,
            handle = layout::HANDLE_OFFSET,
            permanent = layout::state::PERMANENT,
            page = layout::PAGE,
            head = arc_word::HEAD_INLINE,
        )
    }
}

/// The real ARC's helpers (`Module::arc_prompt_death`): the count of a live
/// object of the space is incremented and decremented inline in its in-object word
/// (`dartforge_runtime::layout::arc_word`), the last word of its block, whose size
/// the page publishes in its head; a static object is not counted; anything else —
/// a page without the published size (the audit, an object older than the word), an
/// unregistered or dead object, a release that would reach zero or make a new cycle
/// candidate — calls the runtime.
pub(super) fn prompt_helpers() -> String {
    use dartforge_runtime::layout::{self, arc_word};
    let word = prompt_word;
    let counting = |name: &str, fast: &str, slow: &str| {
        format!(
            r#"define internal void @{name}(i64 %value) alwaysinline nounwind "gc-leaf-function" {{
entry:
{word}{fast}
  br i1 %fast, label %fastpath, label %slow
fastpath:
  store i64 %w1, ptr %wp, align 8
  br label %done
slow:
  call void @{slow}(i64 %value)
  br label %done
done:
  ret void
}}
"#,
            word = word("done", "slow"),
        )
    };
    // A registered, live object: one more occurrence.
    let retain = counting(
        RETAIN,
        &format!(
            "  %live = and i64 %w, {live}
  %fast = icmp ne i64 %live, 0
  %w1 = add i64 %w, 1",
            live = arc_word::LIVE
        ),
        "dartforge_arc_retain",
    );
    // A release that leaves a live object above zero: inline when it is already a
    // cycle candidate or cycle-free; a new candidate is flagged inline and its handle
    // appended to the thread's candidate buffer (`Context::candidates`) while there
    // is room. Zero, a dead or unregistered object, a full buffer: the runtime.
    let release = format!(
        r#"define internal void @{RELEASE}(i64 %value) alwaysinline nounwind "gc-leaf-function" {{
entry:
{word}  %w1 = add i64 %w, -1
  %live = and i64 %w1, {live}
  %rc = and i64 %w1, {rc}
  %marks = and i64 %w1, {marks}
  %l0 = icmp ne i64 %live, 0
  %r0 = icmp ne i64 %rc, 0
  %lr = and i1 %l0, %r0
  br i1 %lr, label %positive, label %slow
positive:
  %m0 = icmp ne i64 %marks, 0
  br i1 %m0, label %fastpath, label %candidate
fastpath:
  store i64 %w1, ptr %wp, align 8
  br label %done
candidate:
  %ctx = call ptr @dartforge_context()
  %np = getelementptr inbounds i8, ptr %ctx, i64 {n}
  %n = load i64, ptr %np, align 8
  %capp = getelementptr inbounds i8, ptr %ctx, i64 {cap}
  %cap = load i64, ptr %capp, align 8
  %room = icmp ult i64 %n, %cap
  br i1 %room, label %push, label %slow
push:
  %bufp = getelementptr inbounds i8, ptr %ctx, i64 {buf}
  %buf = load ptr, ptr %bufp, align 8
  %slot = getelementptr inbounds i64, ptr %buf, i64 %n
  store i64 %value, ptr %slot, align 8
  %n1 = add i64 %n, 1
  store i64 %n1, ptr %np, align 8
  %w2 = or i64 %w1, {candidate}
  store i64 %w2, ptr %wp, align 8
  br label %done
slow:
  call void @dartforge_arc_release(i64 %value)
  br label %done
done:
  ret void
}}
"#,
        word = word("done", "slow"),
        live = arc_word::LIVE,
        rc = arc_word::RC,
        marks = arc_word::CANDIDATE | arc_word::CYCLE_FREE,
        candidate = arc_word::CANDIDATE,
        buf = layout::context::CANDIDATES,
        n = layout::context::N_CANDIDATES,
        cap = layout::context::CANDIDATES_CAP,
    );
    // The owner of an inline counted write: a registered, live object without
    // pending raw writes (`snapshot`).
    let store_ok = format!(
        r#"define internal i1 @{STORE_OK}(i64 %value) alwaysinline nounwind "gc-leaf-function" {{
entry:
{word}  %ls = and i64 %w, {mask}
  %ok = icmp eq i64 %ls, {live}
  ret i1 %ok
no:
  ret i1 false
}}
"#,
        word = word("no", "no"),
        mask = arc_word::LIVE | arc_word::SNAPSHOT,
        live = arc_word::LIVE,
    );
    retain + &release + &store_ok
}

/// The largest instance with a drop function: its reference map is the header's.
pub(super) const DROP_FIELDS: usize = 32;

/// The real ARC's drop function of class `c` with `n` fields (`n` ≤ [`DROP_FIELDS`]):
/// `i64 @df.drop.<c>(i64 %h, ptr %out)` releases each field the header's reference
/// map marks, at its fixed offset — no generic walk of the body. A child that stays
/// above zero, already a candidate or cycle-free, loses its occurrence inline; one
/// that reaches zero is decremented and goes to `out` with bit 0 set (the runtime
/// queues its death); any other goes to `out` as is (the runtime releases it).
/// Returns the entries of `out`. The runtime calls it from the death of an object of
/// the class (`Heap::prompt_die`, registered by `dartforge_arc_register_drops_v1`).
pub(super) fn drop_function(c: i64, n: usize) -> String {
    use dartforge_runtime::layout::{self, arc_word, offsets};
    let mut body = String::new();
    writeln!(
        body,
        "  %b = add i64 %h, -{}\n  %bp = inttoptr i64 %b to ptr\n  %mp = getelementptr inbounds i8, ptr %bp, i64 {}\n  %map = load i32, ptr %mp, align 4\n  br label %f0",
        layout::HANDLE_OFFSET,
        offsets::MAP
    )
    .unwrap();
    let mut k = "0".to_string();
    for i in 0..n {
        writeln!(
            body,
            "f{i}:\n  %k{i} = phi i64 [ {k}, %{pred} ]\n  %bit{i} = and i32 %map, {}\n  %is{i} = icmp ne i32 %bit{i}, 0\n  br i1 %is{i}, label %r{i}, label %f{i}.n\nr{i}:\n  %fa{i} = getelementptr inbounds i8, ptr %bp, i64 {}\n  %v{i} = load i64, ptr %fa{i}, align 8\n  %kr{i} = call i64 @df.arc_drop_child(i64 %v{i}, ptr %out, i64 %k{i})\n  br label %f{i}.n\nf{i}.n:\n  %kn{i} = phi i64 [ %k{i}, %f{i} ], [ %kr{i}, %r{i} ]\n  br label %f{}",
            1u32 << i,
            offsets::BODY + 8 * i,
            i + 1,
            pred = if i == 0 { "entry".to_string() } else { format!("f{}.n", i - 1) },
        )
        .unwrap();
        k = format!("%kn{i}");
    }
    let last = if n == 0 {
        "entry".to_string()
    } else {
        format!("f{}.n", n - 1)
    };
    writeln!(
        body,
        "f{n}:\n  %kend = phi i64 [ {k}, %{last} ]\n  ret i64 %kend"
    )
    .unwrap();
    // `f0` follows `entry` directly; its phi names `entry` as the predecessor.
    let child = format!(
        r#"define internal i64 @df.arc_drop_child(i64 %value, ptr %out, i64 %k) alwaysinline nounwind "gc-leaf-function" {{
entry:
{word}  %w1 = add i64 %w, -1
  %state = and i64 %w1, {live_collecting}
  %live = icmp eq i64 %state, {live}
  br i1 %live, label %counted, label %slow
counted:
  %rc = and i64 %w1, {rc}
  %zero = icmp eq i64 %rc, 0
  br i1 %zero, label %died, label %positive
died:
  store i64 %w1, ptr %wp, align 8
  %slotd = getelementptr inbounds i64, ptr %out, i64 %k
  %tagged = or i64 %value, 1
  store i64 %tagged, ptr %slotd, align 8
  %kd = add i64 %k, 1
  ret i64 %kd
positive:
  %marks = and i64 %w1, {marks}
  %m0 = icmp ne i64 %marks, 0
  br i1 %m0, label %fast, label %slow
fast:
  store i64 %w1, ptr %wp, align 8
  ret i64 %k
slow:
  %slots = getelementptr inbounds i64, ptr %out, i64 %k
  store i64 %value, ptr %slots, align 8
  %ks = add i64 %k, 1
  ret i64 %ks
done:
  ret i64 %k
}}
"#,
        word = prompt_word("done", "slow"),
        live_collecting = arc_word::LIVE | arc_word::COLLECTING,
        live = arc_word::LIVE,
        rc = arc_word::RC,
        marks = arc_word::CANDIDATE | arc_word::CYCLE_FREE,
    );
    format!(
        "{child}define internal i64 @df.drop.{c}(i64 %h, ptr %out) nounwind {{\nentry:\n{body}}}\n"
    )
    .replace(
        "@df.arc_drop_child(i64 %value, ptr %out, i64 %k) alwaysinline",
        "@df.arc_drop_child.{C}(i64 %value, ptr %out, i64 %k) alwaysinline",
    )
    .replace("{C}", &c.to_string())
    .replace(
        "call i64 @df.arc_drop_child(",
        &format!("call i64 @df.arc_drop_child.{c}("),
    )
}

/// The real ARC's test of the owner of an inline counted write
/// ([`prompt_helpers`]).
pub(super) const STORE_OK: &str = "df.arc_store_ok";

#[cfg(test)]
mod tests;
