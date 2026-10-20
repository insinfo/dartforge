//! Verification policy and provenance for optional optimization contracts.
//!
//! Recognition is separate from proof. Until a checker can establish the
//! requested property, verification must fail rather than trust metadata.

/// Closed-HIR evidence for non-publication; emission still requires auditing.
pub mod no_escape;
/// Structural validation for the HIR subset considered by contract analyses.
pub mod ssa;

/// Keeps provenance names distinct for artifacts sharing a stem.
///
/// ```
/// use std::path::{Path, PathBuf};
/// use dartforge_emit_native::contracts::manifest_path;
/// assert_eq!(manifest_path(Path::new("app.arc")), PathBuf::from("app.arc.contracts.json"));
/// ```
pub fn manifest_path(artifact: &std::path::Path) -> std::path::PathBuf {
    let mut name = artifact.file_name().unwrap_or_default().to_os_string();
    name.push(".contracts.json");
    artifact.with_file_name(name)
}

/// The treatment of known DartForge verification requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContractMode {
    /// Require proof for every recognized verification request.
    #[default]
    Verify,
    /// Preserve ordinary Dart behavior without producing contract facts.
    Ignore,
}

impl ContractMode {
    /// Parses the CLI policy without consulting the process environment.
    ///
    /// ```
    /// use dartforge_emit_native::contracts::ContractMode;
    /// assert_eq!(ContractMode::from_name("verify").unwrap(), ContractMode::Verify);
    /// ```
    ///
    /// # Errors
    /// Returns an error when the name is neither `verify` nor `ignore`.
    pub fn from_name(name: &str) -> Result<Self, String> {
        match name {
            "verify" => Ok(Self::Verify),
            "ignore" => Ok(Self::Ignore),
            _ => Err(format!(
                "--dartforge-contracts={name}: expected verify or ignore"
            )),
        }
    }

    /// Returns the stable policy name recorded in compilation artifacts.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Verify => "verify",
            Self::Ignore => "ignore",
        }
    }
}

/// Recognition and verification provenance; this is not a safety certificate.
#[derive(Clone, Debug)]
pub struct ContractReport {
    pub(crate) value: serde_json::Value,
}

impl ContractReport {
    /// Returns machine-readable provenance, including uncertified requests.
    pub fn json(&self) -> &serde_json::Value {
        &self.value
    }

    pub(crate) fn cache_tag(&self) -> String {
        // Keep physical paths in the manifest, but do not make relocatable
        // emission depend on the checkout directory. Ordered source hashes,
        // resolved targets and canonical package URIs remain in the key.
        fn logical(value: &serde_json::Value, key: &str) -> serde_json::Value {
            match value {
                serde_json::Value::Object(fields) => serde_json::Value::Object(
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), logical(value, key)))
                        .collect(),
                ),
                serde_json::Value::Array(values) => serde_json::Value::Array(
                    values.iter().map(|value| logical(value, key)).collect(),
                ),
                serde_json::Value::String(text)
                    if matches!(key, "source" | "uri" | "location")
                        && (text.starts_with("file:")
                            || std::path::Path::new(text).is_absolute()
                            || text.as_bytes().get(1) == Some(&b':')) =>
                {
                    serde_json::Value::String(
                        text.replace('\\', "/")
                            .rsplit('/')
                            .next()
                            .unwrap_or(text)
                            .to_string(),
                    )
                }
                _ => value.clone(),
            }
        }
        blake3::hash(logical(&self.value, "").to_string().as_bytes())
            .to_hex()
            .to_string()
    }
}

pub(crate) fn inspect(
    normalization: &dartforge_types::optimization_annotations::Normalization,
    program: &dartforge_elements::Program,
    mode: ContractMode,
) -> Result<ContractReport, String> {
    use dartforge_types::optimization_annotations::AnnotationKind;
    let location = |unit, span: dartforge_diagnostics::Span| {
        let source = program.unit(unit).source.to_string();
        let before = source.get(..span.start).unwrap_or("");
        let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        format!("{}:{line}:{column}", program.unit_path(unit))
    };
    let mut errors = Vec::new();
    for diagnostic in &normalization.diagnostics {
        // Ignore mode preserves ordinary Dart behavior and records all
        // metadata diagnostics. Dart resolution/constant errors are checked
        // independently before reaching this policy layer.
        if mode == ContractMode::Ignore {
            continue;
        }
        errors.push(format!(
            "{} ({})",
            diagnostic.message,
            location(diagnostic.unit, diagnostic.span)
        ));
    }
    let entries: Vec<_> = normalization.records.iter().map(|record| {
        let preference = matches!(record.kind, AnnotationKind::Preference(_));
        if !preference && mode == ContractMode::Verify {
            errors.push(format!(
                "DartForge contract {:?} is not proved: no contract checker is implemented for this request ({})",
                record.kind, location(record.unit, record.span),
            ));
        }
        serde_json::json!({
            "kind": format!("{:?}", record.kind),
            "target": format!("{:?}", record.target),
            "source": program.unit_path(record.unit),
            "span": {"start": record.span.start, "end": record.span.end},
            "body": format!("{:?}", record.body),
            "status": if preference { "not_applied" } else { "ignored" },
            "reason": if preference { "preference recognized; policy selection is not implemented" }
                      else { "verification explicitly disabled; no safety fact produced" },
            "certified": false,
        })
    }).collect();
    if !errors.is_empty() {
        return Err(format!(
            "DartForge annotation verification failed:\n  {}",
            errors.join("\n  ")
        ));
    }
    let package = normalization.package.as_ref().map(|package| {
        serde_json::json!({
            "defining_uri": package.defining_uri,
            "annotation_schema": package.schema,
            "sources": package.sources.iter().map(|source| serde_json::json!({
                "uri": source.uri,
                "blake3": blake3::hash(source.source.as_bytes()).to_hex().to_string(),
            })).collect::<Vec<_>>(),
        })
    });
    let ignored_diagnostics: Vec<_> = normalization
        .diagnostics
        .iter()
        .map(|d| {
            serde_json::json!({
                "kind": format!("{:?}", d.kind), "message": d.message,
                "location": location(d.unit, d.span), "status": "ignored", "certified": false,
            })
        })
        .collect();
    Ok(ContractReport {
        value: serde_json::json!({
            "manifest_schema": 1, "mode": mode.name(), "certified": false,
            "verification_implemented": false, "package": package,
            "requests": entries, "diagnostics": ignored_diagnostics,
            "sources": program.units.iter().filter(|u| !program.library(u.library).is_sdk)
                .map(|u| serde_json::json!({"uri": u.uri,
                    "blake3": blake3::hash(u.source.as_bytes()).to_hex().to_string(),
                })).collect::<Vec<_>>(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_is_explicit_and_unknown_names_are_rejected() {
        assert_eq!(ContractMode::default(), ContractMode::Verify);
        assert_eq!(
            ContractMode::from_name("ignore").unwrap(),
            ContractMode::Ignore
        );
        assert!(ContractMode::from_name("trust").is_err());
        assert!(ContractMode::from_name("").is_err());
    }

    #[test]
    fn artifact_extensions_do_not_collide_in_provenance_names() {
        use std::path::Path;
        assert_ne!(
            manifest_path(Path::new("app.arc")),
            manifest_path(Path::new("app.tracing"))
        );
        assert_eq!(
            manifest_path(Path::new("app.exe")),
            Path::new("app.exe.contracts.json")
        );
    }

    #[test]
    fn provenance_changes_the_cache_identity_even_without_code_changes() {
        let report = |mode, source| ContractReport {
            value: serde_json::json!({
                "mode": mode, "source": source, "certified": false,
            }),
        };
        assert_ne!(
            report("verify", "a").cache_tag(),
            report("ignore", "a").cache_tag()
        );
        assert_ne!(
            report("ignore", "a").cache_tag(),
            report("ignore", "b").cache_tag()
        );
        let moved = |directory| ContractReport {
            value: serde_json::json!({
                "sources": [{"uri": format!("file:///{directory}/main.dart"), "blake3": "content"}],
            }),
        };
        assert_eq!(moved("first").cache_tag(), moved("second").cache_tag());
    }
}

#[cfg(test)]
#[path = "contracts/integration_tests.rs"]
mod integration_tests;
