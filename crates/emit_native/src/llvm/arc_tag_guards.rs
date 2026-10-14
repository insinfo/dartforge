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

#[cfg(test)]
mod tests;
