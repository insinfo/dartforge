//! Physical ARC dispatch guards; object inventory and logical tokens remain intact.

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

/// The real ARC's helpers (`Module::arc_prompt_death`): the count of a live
/// object of the space is incremented and decremented inline through the metadata
/// array its page publishes (`dartforge_runtime::arc::inline`); a static object
/// is not counted; anything else — an unpublished page, an absent, special or
/// immortal entry, a release that would reach zero or make a new cycle candidate —
/// calls the runtime.
pub(super) fn prompt_helpers() -> String {
    use dartforge_runtime::{arc::inline, layout};
    let lookup = |name: &str, fast_check: &str, op: &str, slow: &str| {
        format!(
            r#"define internal void @{name}(i64 %value) alwaysinline nounwind "gc-leaf-function" {{
entry:
  %nonnull = icmp ne i64 %value, 0
  %tag = and i64 %value, 1
  %even = icmp eq i64 %tag, 0
  %handle = and i1 %nonnull, %even
  br i1 %handle, label %object, label %done
object:
  %b = add i64 %value, -{handle}
  %bp = inttoptr i64 %b to ptr
  %st = load i8, ptr %bp, align 1
  %static = icmp eq i8 %st, {permanent}
  br i1 %static, label %done, label %page
page:
  %pg = and i64 %b, -{page}
  %pgp = inttoptr i64 %pg to ptr
  %metas = load i64, ptr %pgp, align 8
  %unpublished = icmp eq i64 %metas, 0
  br i1 %unpublished, label %slow, label %index
index:
  %geop = getelementptr inbounds i8, ptr %pgp, i64 8
  %geo = load i64, ptr %geop, align 8
  %size = and i64 %geo, 4294967295
  %blocks = lshr i64 %geo, 32
  %off = sub i64 %b, %pg
  %rel = sub i64 %off, {head}
  %ix = udiv i64 %rel, %size
  %inside = icmp ult i64 %ix, %blocks
  br i1 %inside, label %entry2, label %slow
entry2:
  %so = mul i64 %ix, {slot}
  %sa = add i64 %metas, %so
  %sp = inttoptr i64 %sa to ptr
  %fp = getelementptr inbounds i8, ptr %sp, i64 {flags}
  %flags = load i64, ptr %fp, align 8
  %rc = load i64, ptr %sp, align 8
{fast_check}
  br i1 %fast, label %fastpath, label %slow
fastpath:
{op}
  br label %done
slow:
  call void @{slow}(i64 %value)
  br label %done
done:
  ret void
}}
"#,
            handle = layout::HANDLE_OFFSET,
            permanent = layout::state::PERMANENT,
            page = layout::PAGE,
            head = layout::PAGE_HEAD,
            slot = inline::SLOT,
            flags = inline::FLAGS,
        )
    };
    // Bytes `present`, `state`, `immortal` (the low three of the flags word): a
    // present, live (1), non-immortal entry.
    let live = "  %low = and i64 %flags, 16777215
  %live = icmp eq i64 %low, 257
";
    let retain = lookup(
        RETAIN,
        &format!("{live}  %fast = add i1 %live, 0"),
        "  %rc1 = add i64 %rc, 1
  store i64 %rc1, ptr %sp, align 8",
        "dartforge_arc_retain",
    );
    // A release that keeps the count above zero and needs no new cycle candidate
    // (`candidate` or `cycle_free`, bytes 3 and 4).
    let release = lookup(
        RELEASE,
        &format!(
            "{live}  %marks = and i64 %flags, 1099494850560
  %marked = icmp ne i64 %marks, 0
  %above = icmp ugt i64 %rc, 1
  %lm = and i1 %live, %marked
  %fast = and i1 %lm, %above"
        ),
        "  %rc1 = sub i64 %rc, 1
  store i64 %rc1, ptr %sp, align 8",
        "dartforge_arc_release",
    );
    // The owner of an inline counted write: a present, live, non-immortal entry
    // without pending raw writes (`snapshot`).
    let store_ok = format!(
        r#"define internal i1 @{STORE_OK}(i64 %value) alwaysinline nounwind "gc-leaf-function" {{
entry:
  %nonnull = icmp ne i64 %value, 0
  %tag = and i64 %value, 1
  %even = icmp eq i64 %tag, 0
  %handle = and i1 %nonnull, %even
  br i1 %handle, label %object, label %no
object:
  %b = add i64 %value, -{handle}
  %bp = inttoptr i64 %b to ptr
  %st = load i8, ptr %bp, align 1
  %static = icmp eq i8 %st, {permanent}
  br i1 %static, label %no, label %page
page:
  %pg = and i64 %b, -{page}
  %pgp = inttoptr i64 %pg to ptr
  %metas = load i64, ptr %pgp, align 8
  %unpublished = icmp eq i64 %metas, 0
  br i1 %unpublished, label %no, label %index
index:
  %geop = getelementptr inbounds i8, ptr %pgp, i64 8
  %geo = load i64, ptr %geop, align 8
  %size = and i64 %geo, 4294967295
  %blocks = lshr i64 %geo, 32
  %off = sub i64 %b, %pg
  %rel = sub i64 %off, {head}
  %ix = udiv i64 %rel, %size
  %inside = icmp ult i64 %ix, %blocks
  br i1 %inside, label %entry2, label %no
entry2:
  %so = mul i64 %ix, {slot}
  %sa = add i64 %metas, %so
  %sp = inttoptr i64 %sa to ptr
  %fp = getelementptr inbounds i8, ptr %sp, i64 {flags}
  %flags = load i64, ptr %fp, align 8
  %low = and i64 %flags, 16777215
  %live = icmp eq i64 %low, 257
  %snp = getelementptr inbounds i8, ptr %sp, i64 {snapshot}
  %sn = load i8, ptr %snp, align 1
  %clean = icmp eq i8 %sn, 0
  %ok = and i1 %live, %clean
  ret i1 %ok
no:
  ret i1 false
}}
"#,
        handle = layout::HANDLE_OFFSET,
        permanent = layout::state::PERMANENT,
        page = layout::PAGE,
        head = layout::PAGE_HEAD,
        slot = inline::SLOT,
        flags = inline::FLAGS,
        snapshot = inline::SNAPSHOT,
    );
    retain + &release + &store_ok
}

/// The real ARC's test of the owner of an inline counted write
/// ([`prompt_helpers`]).
pub(super) const STORE_OK: &str = "df.arc_store_ok";

#[cfg(test)]
mod tests;
