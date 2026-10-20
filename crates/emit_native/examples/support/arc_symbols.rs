//! Select only emitter-managed ARC call targets; handwritten harness calls stay raw.

/// Matches the actual emitted helper definition, never a harness call site.
pub(super) fn source_symbol(ir: &str, operation: &str) -> &'static str {
    match operation {
        "retain" if ir.contains("define internal void @df.arc_retain_ref(") => "df.arc_retain_ref",
        "retain" => "dartforge_arc_retain",
        "release" if ir.contains("define internal void @df.arc_release_ref(") => {
            "df.arc_release_ref"
        }
        "release" => "dartforge_arc_release",
        _ => panic!("unsupported ARC proof operation"),
    }
}
