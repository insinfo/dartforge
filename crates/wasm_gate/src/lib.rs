//! The binary gate of DartForge's `wasm32-mvp` profile (docs/PLANO-WASM-MVP.md
//! §3.1, phase W0).
//!
//! A module passes only when
//!
//! * `wasmparser`, configured with exactly [`WasmFeatures::MVP`] (the original
//!   feature set: no sign extension, saturating conversions, bulk memory,
//!   reference types, multi-value, SIMD, threads, exceptions, tail calls,
//!   memory64, multi-memory, GC nor import/export of mutable globals),
//!   validates every instruction, type and section;
//! * every import is a function of an allowed module (the host ABI namespace,
//!   `dartforge_host_v1` by default): no imported memory, table or global, so
//!   the module owns its linear memory and nothing from WASI, libc or the
//!   engine sneaks in;
//! * it defines at most one memory and one table, and exports every symbol the
//!   policy requires.
//!
//! The `target_features` custom section LLVM writes is reported, never trusted:
//! the validator decides from the code itself. The report carries the SHA-256 of
//! the exact bytes, so "the same module" in two hosts is checked by hash.

use sha2::{Digest, Sha256};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef, Validator, WasmFeatures};

/// The host ABI namespace of the first version (docs/PLANO-WASM-MVP.md §3.4).
pub const HOST_MODULE: &str = "dartforge_host_v1";

/// What a module may import and must export.
#[derive(Clone, Debug)]
pub struct Policy {
    /// Modules functions may be imported from.
    pub import_modules: Vec<String>,
    /// Exports that must be present (functions or the memory).
    pub required_exports: Vec<String>,
    /// The largest initial memory, in 64 KiB pages.
    pub max_initial_pages: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            import_modules: vec![HOST_MODULE.to_string()],
            required_exports: vec!["memory".to_string()],
            max_initial_pages: 16 * 1024,
        }
    }
}

/// What a valid module looks like from outside.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub sha256: String,
    pub bytes: usize,
    /// `(module, name)` of each imported function.
    pub imports: Vec<(String, String)>,
    /// `(name, kind)` of each export.
    pub exports: Vec<(String, String)>,
    pub defined_functions: u32,
    pub memories: u32,
    pub tables: u32,
    /// Initial and maximum pages of the memory, when it has one.
    pub memory_pages: Option<(u64, Option<u64>)>,
    /// The features named by LLVM's `target_features` section (informative).
    pub declared_features: Vec<String>,
}

impl Report {
    /// The report as JSON, for evidence files.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "profile": "wasm32-mvp",
            "validator": "wasmparser 0.262.0 WasmFeatures::MVP",
            "sha256": self.sha256,
            "bytes": self.bytes,
            "imports": self.imports.iter().map(|(m, n)| format!("{m}.{n}")).collect::<Vec<_>>(),
            "exports": self.exports.iter().map(|(n, k)| format!("{n}:{k}")).collect::<Vec<_>>(),
            "defined_functions": self.defined_functions,
            "memories": self.memories,
            "tables": self.tables,
            "memory_pages": self.memory_pages.map(|(i, m)| serde_json::json!({"initial": i, "maximum": m})),
            "declared_features": self.declared_features,
        })
    }
}

fn kind_name(kind: ExternalKind) -> &'static str {
    match kind {
        ExternalKind::Func | ExternalKind::FuncExact => "func",
        ExternalKind::Table => "table",
        ExternalKind::Memory => "memory",
        ExternalKind::Global => "global",
        ExternalKind::Tag => "tag",
    }
}

/// The SHA-256 of `bytes`, in lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Validates `bytes` against the strict MVP profile and `policy`. Every
/// violation is listed, so a failing gate explains itself.
///
/// # Errors
/// The violations, when there is any.
pub fn validate(bytes: &[u8], policy: &Policy) -> Result<Report, Vec<String>> {
    let mut errors = Vec::new();
    if let Err(e) = Validator::new_with_features(WasmFeatures::MVP).validate_all(bytes) {
        errors.push(format!("not valid in the MVP feature set: {e}"));
        return Err(errors);
    }
    let mut report = Report {
        sha256: sha256_hex(bytes),
        bytes: bytes.len(),
        ..Report::default()
    };
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = match payload {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("unreadable section: {e}"));
                continue;
            }
        };
        match payload {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let Ok(import) = import else {
                        errors.push("unreadable import".to_string());
                        continue;
                    };
                    let module = import.module.to_string();
                    let name = import.name.to_string();
                    match import.ty {
                        TypeRef::Func(_) | TypeRef::FuncExact(_) => {
                            if !policy.import_modules.contains(&module) {
                                errors.push(format!(
                                    "import {module}.{name} outside the host ABI ({})",
                                    policy.import_modules.join(", ")
                                ));
                            }
                            report.imports.push((module, name));
                        }
                        TypeRef::Memory(_) => errors.push(format!(
                            "imported memory {module}.{name}: the module owns its linear memory"
                        )),
                        TypeRef::Table(_) => errors.push(format!("imported table {module}.{name}")),
                        TypeRef::Global(_) => {
                            errors.push(format!("imported global {module}.{name}"))
                        }
                        TypeRef::Tag(_) => errors.push(format!("imported tag {module}.{name}")),
                    }
                }
            }
            Payload::FunctionSection(section) => report.defined_functions = section.count(),
            Payload::TableSection(section) => report.tables = section.count(),
            Payload::MemorySection(section) => {
                report.memories = section.count();
                for memory in section {
                    let Ok(memory) = memory else { continue };
                    if memory.initial > policy.max_initial_pages {
                        errors.push(format!(
                            "initial memory of {} pages above the limit of {}",
                            memory.initial, policy.max_initial_pages
                        ));
                    }
                    report.memory_pages = Some((memory.initial, memory.maximum));
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let Ok(export) = export else {
                        errors.push("unreadable export".to_string());
                        continue;
                    };
                    if export.kind == ExternalKind::Global {
                        errors.push(format!(
                            "exported global {}: the host ABI exports functions and the memory",
                            export.name
                        ));
                    }
                    report
                        .exports
                        .push((export.name.to_string(), kind_name(export.kind).to_string()));
                }
            }
            Payload::CustomSection(section) if section.name() == "target_features" => {
                report.declared_features = target_features(section.data());
            }
            _ => {}
        }
    }
    if report.memories > 1 || report.tables > 1 {
        errors.push("more than one memory or table".to_string());
    }
    for required in &policy.required_exports {
        if !report.exports.iter().any(|(n, _)| n == required) {
            errors.push(format!("missing export {required}"));
        }
    }
    if errors.is_empty() {
        Ok(report)
    } else {
        Err(errors)
    }
}

/// The entries of a `target_features` section (`+name`, `-name`, `=name`).
fn target_features(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut reader = wasmparser::BinaryReader::new(data, 0);
    let Ok(count) = reader.read_var_u32() else {
        return out;
    };
    for _ in 0..count {
        let (Ok(prefix), Ok(name)) = (reader.read_u8(), reader.read_string()) else {
            break;
        };
        out.push(format!("{}{name}", prefix as char));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(text: &str) -> Vec<u8> {
        wat::parse_str(text).expect("valid text fixture")
    }

    fn policy() -> Policy {
        Policy {
            required_exports: vec!["memory".to_string(), "run".to_string()],
            ..Policy::default()
        }
    }

    const GOOD: &str = r#"(module
        (import "dartforge_host_v1" "write" (func $write (param i32 i32 i32) (result i32)))
        (memory (export "memory") 1)
        (table 1 funcref)
        (global $sp (mut i32) (i32.const 1024))
        (func $twice (param i64) (result i64) (i64.add (local.get 0) (local.get 0)))
        (elem (i32.const 0) $twice)
        (type $t (func (param i64) (result i64)))
        (func (export "run") (result i64)
          (call_indirect (type $t) (i64.const 21) (i32.const 0))))"#;

    #[test]
    fn mvp_module_with_host_imports_passes() {
        let r = validate(&module(GOOD), &policy()).expect("MVP module");
        assert_eq!(
            r.imports,
            vec![("dartforge_host_v1".into(), "write".into())]
        );
        assert_eq!((r.memories, r.tables), (1, 1));
        assert!(r.exports.contains(&("run".into(), "func".into())));
        assert_eq!(r.sha256.len(), 64);
    }

    /// Each control must fail: one post-MVP feature or ABI violation at a time.
    #[test]
    fn negative_controls_fail() {
        let cases: &[(&str, &str, &str)] = &[
            (
                "sign-extension",
                "MVP",
                r#"(module (memory (export "memory") 1)
                   (func (export "run") (result i64) (i64.extend8_s (i64.const 1))))"#,
            ),
            (
                "saturating conversion",
                "MVP",
                r#"(module (memory (export "memory") 1)
                   (func (export "run") (result i64) (i64.trunc_sat_f64_s (f64.const 1))))"#,
            ),
            (
                "bulk memory",
                "MVP",
                r#"(module (memory (export "memory") 1)
                   (func (export "run") (result i64)
                     (memory.fill (i32.const 0) (i32.const 0) (i32.const 1)) (i64.const 0)))"#,
            ),
            (
                "multi-value",
                "MVP",
                r#"(module (memory (export "memory") 1)
                   (func (result i64 i64) (i64.const 1) (i64.const 2))
                   (func (export "run") (result i64) (i64.const 0)))"#,
            ),
            (
                "mutable global export",
                "MVP",
                r#"(module (memory (export "memory") 1)
                   (global (export "g") (mut i32) (i32.const 0))
                   (func (export "run") (result i64) (i64.const 0)))"#,
            ),
            (
                "WASI import",
                "outside the host ABI",
                r#"(module
                   (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
                   (memory (export "memory") 1)
                   (func (export "run") (result i64) (i64.const 0)))"#,
            ),
            (
                "imported memory",
                "imported memory",
                r#"(module (import "dartforge_host_v1" "memory" (memory 1))
                   (export "memory" (memory 0))
                   (func (export "run") (result i64) (i64.const 0)))"#,
            ),
            (
                "missing export",
                "missing export run",
                r#"(module (memory (export "memory") 1))"#,
            ),
        ];
        for (name, reason, text) in cases {
            // Texts with post-MVP instructions still parse: `wat` knows them.
            let bytes = module(text);
            let errors = validate(&bytes, &policy())
                .expect_err(&format!("the gate accepted the control {name}"));
            assert!(
                errors.iter().any(|e| e.contains(reason)),
                "{name} failed for another reason: {errors:?}"
            );
        }
    }

    #[test]
    fn target_features_are_reported_not_trusted() {
        // `+sign-ext` declared, but no such instruction used: the code decides.
        let mut bytes = module(GOOD);
        let mut section = vec![1u8, b'+', 8];
        section.extend_from_slice(b"sign-ext");
        let name = b"target_features";
        let mut payload = vec![name.len() as u8];
        payload.extend_from_slice(name);
        payload.extend_from_slice(&section);
        bytes.push(0);
        bytes.push(payload.len() as u8);
        bytes.extend_from_slice(&payload);
        let r = validate(&bytes, &policy()).expect("declared features do not decide");
        assert_eq!(r.declared_features, vec!["+sign-ext".to_string()]);
    }
}
