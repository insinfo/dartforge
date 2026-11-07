//! Opaque compilation-context binding for native program IR and SDK images.
//! The compiler owns the identity schema; the executor only compares identities.
use std::path::Path;

/// Single-line program context header, ignored by LLVM.
pub const IR_PREFIX: &str = "; DartForge compilation environment ";
/// Plain UTF-8 identity next to a source SDK image, written before publication.
pub const SDK_IDENTITY_FILE: &str = "compilation-environment.identity";

/// Reads a unique opaque context identity without interpreting compiler data.
///
/// # Errors
/// An empty identity or multiple context headers.
///
/// ```
/// use dartforge_abi::compilation_environment::ir_identity;
/// assert_eq!(ir_identity("; legacy IR\n").unwrap(), None);
/// ```
pub fn ir_identity(ir: &str) -> Result<Option<&str>, String> {
    let mut headers = ir.lines().filter_map(|line| line.strip_prefix(IR_PREFIX));
    let identity = headers.next();
    if identity.is_some_and(str::is_empty) {
        return Err("empty compilation environment identity in LLVM IR".into());
    }
    if headers.next().is_some() {
        return Err("multiple compilation environment headers in LLVM IR".into());
    }
    Ok(identity)
}

/// Requires an SDK image's published identity to match its program's context.
/// Legacy IR without an identity keeps its earlier loading contract.
///
/// # Errors
/// Missing, unreadable or mismatched SDK metadata.
///
/// ```
/// use std::path::Path;
/// assert!(dartforge_abi::compilation_environment::validate_sdk_identity(Path::new("legacy.dll"), None).is_ok());
/// ```
pub fn validate_sdk_identity(dll: &Path, identity: Option<&str>) -> Result<(), String> {
    let Some(expected) = identity else {
        return Ok(());
    };
    let manifest = dll
        .parent()
        .unwrap_or(Path::new("."))
        .join(SDK_IDENTITY_FILE);
    let actual = std::fs::read_to_string(&manifest).map_err(|error| {
        format!(
            "source SDK environment metadata {}: {error}",
            manifest.display()
        )
    })?;
    if actual != expected {
        return Err("source SDK and program compilation environments do not match".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_headers_are_unique_and_opaque() {
        let header = format!("{IR_PREFIX}[\"context\",{{\"a\":\"b\"}}]\r\n; next line\n");
        assert_eq!(
            ir_identity(&header).unwrap(),
            Some("[\"context\",{\"a\":\"b\"}]")
        );
        assert_eq!(ir_identity("; helper module\n").unwrap(), None);
        assert!(ir_identity(&format!("{IR_PREFIX}\n")).is_err());
        assert!(ir_identity(&format!("{IR_PREFIX}a\n{IR_PREFIX}a\n")).is_err());
    }

    #[test]
    fn sdk_metadata_must_match_exactly_and_legacy_ir_stays_supported() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "dartforge-abi-context-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let dll = directory.join("sdk.dll");
        assert!(validate_sdk_identity(&dll, None).is_ok());
        assert!(validate_sdk_identity(&dll, Some("expected")).is_err());
        let manifest = directory.join(SDK_IDENTITY_FILE);
        std::fs::write(&manifest, "different").unwrap();
        assert!(validate_sdk_identity(&dll, Some("expected")).is_err());
        std::fs::write(&manifest, "expected").unwrap();
        assert!(validate_sdk_identity(&dll, Some("expected")).is_ok());
        std::fs::write(&manifest, "expected\n").unwrap();
        assert!(validate_sdk_identity(&dll, Some("expected")).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
