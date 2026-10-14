//! Exercise recognition, diagnostics and provenance through native emission.

use super::*;
use crate::{CompileOptions, emit_ir};

const API: &str = "const int dartForgeAnnotationsSchema = 1; final class PreferRegion { const PreferRegion(); } final class NoEscape { const NoEscape(); } final class Realtime { const Realtime(); }";

#[test]
fn canonical_requests_fail_closed_and_ignore_is_explicit() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let directory = tempfile::tempdir().unwrap();
        let package = directory.path().join("pkg/lib/src");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("annotations.dart"), API).unwrap();
        let config = directory.path().join("packages.json");
        std::fs::write(&config, r#"{"configVersion":2,"packages":[{"name":"dartforge","rootUri":"pkg/","packageUri":"lib/","languageVersion":"3.6"}]}"#).unwrap();
        let entry = directory.path().join("main.dart");
        std::fs::write(&entry, "import 'package:dartforge/src/annotations.dart'; int f(@NoEscape() Object value) => 3; @Realtime() int g(int value) => value + 1; void main() { print(g(f(1))); }").unwrap();
        let mut options = CompileOptions {
            contracts: ContractMode::Verify, sdk: Some(crate::sdk_tests()), packages: Some(&config),
            timings: false, optimize: false, language_version: None, experiments: vec![],
            debug: false, generator: None, cpu: None,
        };
        let error = emit_ir(&entry, &options).unwrap_err();
        assert!(error.contains("NoEscape") && error.contains("Realtime"), "{error}");
        assert!(error.contains("is not proved") && error.contains("main.dart:1:"), "{error}");
        options.contracts = ContractMode::Ignore;
        let emitted = emit_ir(&entry, &options).unwrap();
        let report = emitted.contracts.json();
        assert_eq!(report["mode"], "ignore");
        assert_eq!(report["certified"], false);
        let requests = report["requests"].as_array().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r["status"] == "ignored" && r["certified"] == false));

        std::fs::write(&entry, "import 'package:dartforge/src/annotations.dart'; @PreferRegion() int f(int value) => value + 1; void main() { print(f(1)); }").unwrap();
        options.contracts = ContractMode::Verify;
        let preferred = emit_ir(&entry, &options).unwrap();
        assert_eq!(preferred.contracts.json()["requests"][0]["status"], "not_applied");
        options.contracts = ContractMode::Ignore;
        let ignored = emit_ir(&entry, &options).unwrap();
        assert_ne!(preferred.contracts.cache_tag(), ignored.contracts.cache_tag());
        assert_ne!(preferred.text, ignored.text, "policy must bind object-cache input");

        std::fs::write(package.join("annotations.dart"), API.replace("Schema = 1", "Schema = 2")).unwrap();
        options.contracts = ContractMode::Verify;
        assert!(emit_ir(&entry, &options).unwrap_err().contains("schema must be 1"));
        options.contracts = ContractMode::Ignore;
        let unsupported = emit_ir(&entry, &options).unwrap();
        assert_eq!(unsupported.contracts.json()["diagnostics"][0]["status"], "ignored");
        assert_eq!(unsupported.contracts.json()["package"]["annotation_schema"], 2);
        assert_ne!(ignored.contracts.cache_tag(), unsupported.contracts.cache_tag());
    }).unwrap().join().unwrap();
}

#[test]
fn matching_annotation_names_in_other_libraries_have_no_authority() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("fake.dart");
        std::fs::write(&entry, "library dartforge.src.annotations; class Realtime { const Realtime(); } @Realtime() int f() => 3; void main() { print(f()); }").unwrap();
        let options = CompileOptions {
            contracts: ContractMode::Verify, sdk: Some(crate::sdk_tests()), packages: None,
            timings: false, optimize: false, language_version: None, experiments: vec![],
            debug: false, generator: None, cpu: None,
        };
        let ir = emit_ir(&entry, &options).unwrap();
        assert_eq!(ir.contracts.json()["requests"], serde_json::json!([]));
        assert!(ir.contracts.json()["package"].is_null());
    }).unwrap().join().unwrap();
}
