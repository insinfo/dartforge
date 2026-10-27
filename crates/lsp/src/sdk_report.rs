//! Which Dart SDK the server analyzes against, and how the client learns it.
//!
//! The binary discovers the SDK at startup
//! ([`dartforge_elements::sdk_discovery::search`]: `DARTFORGE_SDK_LIB`, the
//! SDK next to the executable, `DART_SDK`, `FLUTTER_ROOT`, `dart`/`flutter` on
//! the `PATH`, well-known install locations); an `initializationOptions.sdkPath`
//! sent by the client overrides it. After `initialized`, the server says once
//! through `window/logMessage` which SDK it uses (or what it tried) and, without
//! any SDK, shows a single `window/showMessage` warning explaining how to
//! configure one: without an SDK only the syntactic features answer.

use dartforge_elements::sdk_discovery::{FoundSdk, SdkSearch};
use serde_json::{Value, json};

/// The language version whose semantics the analyzer reproduces.
const TARGET_VERSION: &str = "3.6";

/// The SDK outcome reported to the client after `initialized`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SdkReport {
    /// The SDK in use, if any.
    pub chosen: Option<FoundSdk>,
    /// Refused candidates, in the order they were looked at.
    pub rejected: Vec<String>,
    /// The client configured an SDK (`initializationOptions.sdkPath`) the
    /// server could not use: the reason.
    pub configured_error: Option<String>,
}

impl SdkReport {
    /// The report of a discovery whose chosen SDK loaded (`load_error: None`)
    /// or failed to load (the reason).
    pub fn from_search(search: SdkSearch, load_error: Option<String>) -> Self {
        let mut report = SdkReport {
            chosen: None,
            rejected: search.rejected,
            configured_error: None,
        };
        match (search.found, load_error) {
            (Some(found), None) => report.chosen = Some(found),
            (Some(found), Some(e)) => report.rejected.push(format!(
                "{}: {} (could not be loaded: {e})",
                found.source,
                found.lib.display()
            )),
            (None, _) => {}
        }
        report
    }

    /// The `window/logMessage` and `window/showMessage` notifications that
    /// describe the report, in order.
    ///
    /// ```
    /// use dartforge_lsp::SdkReport;
    /// let messages = SdkReport::default().messages();
    /// assert!(messages.iter().any(|m| m["method"] == "window/showMessage"));
    /// ```
    pub fn messages(&self) -> Vec<Value> {
        let mut out = Vec::new();
        if let Some(e) = &self.configured_error {
            out.push(notification(
                "window/showMessage",
                WARNING,
                &format!("DartForge: the Dart SDK set in the client configuration (sdkPath) is not usable: {e}."),
            ));
        }
        match &self.chosen {
            Some(sdk) => {
                let root = sdk.lib.parent().unwrap_or(&sdk.lib);
                let version = sdk.version.as_deref().unwrap_or("unknown version");
                out.push(notification(
                    "window/logMessage",
                    INFO,
                    &format!(
                        "Using the Dart SDK at {} ({version}), found through {}.",
                        root.display(),
                        sdk.source
                    ),
                ));
                if let Some(v) = &sdk.version
                    && !matches_target(v)
                {
                    out.push(notification(
                        "window/logMessage",
                        WARNING,
                        &format!(
                            "DartForge analyzes with Dart 3.6.2 semantics; the libraries of Dart {v} may give different results. \
                             Point \"dartforge.sdkPath\" (or DARTFORGE_SDK_LIB) at a Dart 3.6.2 SDK for exact parity."
                        ),
                    ));
                }
                if !self.rejected.is_empty() {
                    out.push(notification(
                        "window/logMessage",
                        LOG,
                        &format!("Dart SDK candidates skipped: {}", self.rejected.join("; ")),
                    ));
                }
            }
            None => {
                let tried = if self.rejected.is_empty() {
                    "no candidate found".to_string()
                } else {
                    self.rejected.join("; ")
                };
                out.push(notification(
                    "window/logMessage",
                    WARNING,
                    &format!("No Dart SDK found ({tried}). Only syntactic features are available."),
                ));
                out.push(notification(
                    "window/showMessage",
                    WARNING,
                    "DartForge could not find a Dart SDK, so hover, completion, navigation, rename and semantic highlighting \
                     are unavailable (syntax errors, formatting and the outline still work). Install the Dart SDK and put \
                     `dart` on the PATH, set DART_SDK, or set the VS Code setting \"dartforge.sdkPath\" to the SDK folder \
                     (other editors: the DARTFORGE_SDK_LIB environment variable or the sdkPath initialization option).",
                ));
            }
        }
        out
    }
}

/// `3.6`, `3.6.2`, `3.6.2-…` match the analyzer's target.
fn matches_target(version: &str) -> bool {
    version == TARGET_VERSION
        || version
            .strip_prefix(TARGET_VERSION)
            .is_some_and(|rest| rest.starts_with('.'))
}

const WARNING: u8 = 2;
const INFO: u8 = 3;
const LOG: u8 = 4;

fn notification(method: &str, ty: u8, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": {"type": ty, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn found(version: Option<&str>) -> FoundSdk {
        FoundSdk {
            lib: PathBuf::from("/sdk/lib"),
            source: "DART_SDK".into(),
            version: version.map(str::to_string),
        }
    }

    #[test]
    fn a_found_sdk_is_logged_without_a_popup() {
        let report = SdkReport {
            chosen: Some(found(Some("3.6.2"))),
            ..Default::default()
        };
        let m = report.messages();
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m[0]["method"], "window/logMessage");
        assert!(
            m[0]["params"]["message"]
                .as_str()
                .unwrap()
                .contains("DART_SDK")
        );
    }

    #[test]
    fn a_newer_sdk_adds_a_parity_warning() {
        let report = SdkReport {
            chosen: Some(found(Some("3.13.5"))),
            ..Default::default()
        };
        let m = report.messages();
        assert_eq!(m.len(), 2);
        assert_eq!(m[1]["params"]["type"], 2);
        assert!(matches_target("3.6"));
        assert!(matches_target("3.6.2-beta"));
        assert!(!matches_target("3.60.0"));
    }

    #[test]
    fn no_sdk_shows_one_warning_naming_the_setting() {
        let report = SdkReport {
            rejected: vec!["DART_SDK: /x (does not exist)".into()],
            ..Default::default()
        };
        let m = report.messages();
        let shown: Vec<_> = m
            .iter()
            .filter(|n| n["method"] == "window/showMessage")
            .collect();
        assert_eq!(shown.len(), 1);
        assert!(
            shown[0]["params"]["message"]
                .as_str()
                .unwrap()
                .contains("dartforge.sdkPath")
        );
        assert!(
            m[0]["params"]["message"]
                .as_str()
                .unwrap()
                .contains("DART_SDK: /x")
        );
    }
}
