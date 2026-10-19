//! Detect definitions with the same YAML document semantics as configuration.
use std::path::Path;
use yaml_rust2::Yaml;

pub(super) fn requires_engine(path: &Path) -> bool {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        // An unreadable configuration is not evidence of absent builders.
        // The engine's strict configuration reader reports the actual error.
        Err(_) => return true,
    };
    let document = match crate::config::load_yaml(&text, "build.yaml") {
        Ok(document) => document,
        Err(_) => return true,
    };
    match &document {
        Yaml::Null => false,
        Yaml::Hash(_) => ["builders", "post_process_builders"]
            .into_iter()
            .any(|key| {
                match &document[key] {
                    Yaml::Null | Yaml::BadValue => false,
                    Yaml::Hash(definitions) => !definitions.is_empty(),
                    // Wrongly typed sections must reach validation instead of
                    // allowing compilation to use a stale generated disk file.
                    _ => true,
                }
            }),
        _ => true,
    }
}
