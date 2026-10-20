//! Integrity-bound publication of direct obligations and allocation closure.
//!
//! The legacy `.poda` records and return-CID certificate schema stay unchanged.
//! Its separate ready marker binds
//! all facts to the compiler, exact runtime, finalized IR, ABI and SDK artifacts.
use super::closure::Bundle;
use serde_json::{Value, json};
use std::path::Path;
mod shape;
pub(super) const SCHEMA: u64 = 5;
const FILE: &str = "allocation-inventory.json";
const READY: &str = "allocation-inventory.ready";
fn digest(path: &Path) -> Option<String> {
    super::integrity_io::digest_file(path)
}
fn runtime_digest(runtime: &Path) -> Option<Value> {
    Some(json!({"artifact_digest":digest(runtime)?,
        "effect_catalog_digest":blake3::hash(include_bytes!("../../../../runtime/effects.tsv")).to_hex().to_string()}))
}
fn artifacts(directory: &Path, names: &[String]) -> Option<Vec<Value>> {
    let mut seen = std::collections::HashSet::new();
    names
        .iter()
        .map(|name| {
            let path = Path::new(name);
            if path.components().count() != 1
                || path.file_name()?.to_str()? != name
                || !seen.insert(name)
            {
                return None;
            }
            Some(json!({"name":name,"digest":digest(&directory.join(path))?}))
        })
        .collect()
}

pub(crate) fn publish(
    directory: &Path,
    key: &str,
    production: bool,
    names: &[String],
    runtime: &Path,
    bundle: &Bundle,
) -> Result<(), String> {
    let artifacts = artifacts(directory, names)
        .ok_or_else(|| "cannot bind allocation inventory SDK artifacts".to_string())?;
    let runtime = runtime_digest(runtime)
        .ok_or_else(|| "cannot bind allocation inventory runtime artifact".to_string())?;
    let value = json!({"schema":SCHEMA,"cache_key":key,"production":production,
        "compiler":env!("DARTFORGE_EMITTER_HASH"),"runtime":runtime,"artifacts":artifacts,"inventory":bundle.to_json()});
    let file = std::fs::File::create(directory.join(FILE)).map_err(|error| error.to_string())?;
    let digest = super::integrity_io::write_json(file, &value).map_err(|error| error.to_string())?;
    let ready = json!({"schema":SCHEMA,"cache_key":key,"inventory_digest":digest.to_hex().to_string()});
    std::fs::write(directory.join(READY), format!("{}\n", ready)).map_err(|error| error.to_string())
}

/// Checks publication integrity only. It does not certify imports, exposure,
/// complete class reachability or turn missing records into empty facts.
pub(crate) fn verify(
    directory: &Path,
    key: &str,
    production: bool,
    names: &[String],
    runtime: &Path,
) -> bool {
    let checked = || -> Option<()> {
        let text = std::fs::read(directory.join(FILE)).ok()?;
        let ready: Value =
            serde_json::from_slice(&std::fs::read(directory.join(READY)).ok()?).ok()?;
        if ready
            != json!({"schema":SCHEMA,"cache_key":key,"inventory_digest":blake3::hash(&text).to_hex().to_string()})
            || !super::integrity_io::canonical_document_matches(&text)
        {
            return None;
        }
        let envelope: shape::Envelope = serde_json::from_slice(&text).ok()?;
        let shape::InventoryShape = envelope.inventory;
        if envelope.schema != SCHEMA
            || envelope.cache_key != key
            || envelope.production != production
            || envelope.compiler != env!("DARTFORGE_EMITTER_HASH")
            || envelope.runtime != runtime_digest(runtime)?
            || envelope.artifacts != json!(artifacts(directory, names)?)
        {
            return None;
        }
        Some(())
    };
    checked().is_some()
}

/// Return the verified payload from the same read used to check its ready marker.
/// Integrity alone is not an allocation or exposure certificate.
pub(super) fn read_verified(
    directory: &Path,
    key: &str,
    production: bool,
    names: &[String],
    runtime: &Path,
) -> Option<Value> {
    let checked = || -> Option<Value> {
        let text = std::fs::read_to_string(directory.join(FILE)).ok()?;
        let ready: Value =
            serde_json::from_slice(&std::fs::read(directory.join(READY)).ok()?).ok()?;
        let value: Value = serde_json::from_str(&text).ok()?;
        if ready
            != json!({"schema":SCHEMA,"cache_key":key,"inventory_digest":blake3::hash(text.as_bytes()).to_hex().to_string()})
            || !super::integrity_io::canonical_matches(text.as_bytes(), &value)
            || value.as_object()?.len() != 7
            || value["schema"] != SCHEMA
            || value["cache_key"] != key
            || value["production"] != production
            || value["compiler"] != env!("DARTFORGE_EMITTER_HASH")
            || value["runtime"] != runtime_digest(runtime)?
            || value["artifacts"] != json!(artifacts(directory, names)?)
            || value["inventory"].as_object()?.len() != 4
            || !value["inventory"]["functions"].is_array()
            || !value["inventory"]["globals"].is_array()
            || !value["inventory"]["modules"].is_array()
        {
            return None;
        }
        Some(value)
    };
    checked()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_verify_envelope_matches_the_full_value_contract() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("runtime.a");
        std::fs::write(&runtime, b"runtime").unwrap();
        publish(
            dir.path(),
            "key",
            true,
            &[],
            &runtime,
            &super::super::closure::audit(&[]),
        )
        .unwrap();
        let original: Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(FILE)).unwrap()).unwrap();
        for variant in 0..14 {
            let mut value = original.clone();
            let expected = match variant {
                0 => true,
                1 => {
                    value["inventory"]["functions"] = json!([null,{"uninterpreted":"record"}]);
                    true
                }
                2 => {
                    let map = value["inventory"].as_object_mut().unwrap();
                    map.remove("scope");
                    map.insert("other".into(), json!(null));
                    true
                }
                3 => {
                    value["inventory"]["functions"] = json!({});
                    false
                }
                4 => {
                    value["inventory"]["extra"] = json!(0);
                    false
                }
                5 => {
                    value["inventory"]
                        .as_object_mut()
                        .unwrap()
                        .remove("globals");
                    value["inventory"]["other"] = json!([]);
                    false
                }
                6 => {
                    value["cache_key"] = json!("other");
                    false
                }
                7 => {
                    value["production"] = json!(false);
                    false
                }
                8 => {
                    value["schema"] = json!(0);
                    false
                }
                9 => {
                    value["compiler"] = json!("other");
                    false
                }
                10 => {
                    value["runtime"] = json!({});
                    false
                }
                11 => {
                    value["artifacts"] = json!([{"name":"missing","digest":"bad"}]);
                    false
                }
                12 => {
                    value["extra"] = json!(0);
                    false
                }
                _ => {
                    value.as_object_mut().unwrap().remove("inventory");
                    false
                }
            };
            let text = format!("{value}\n");
            std::fs::write(dir.path().join(FILE), &text).unwrap();
            std::fs::write(dir.path().join(READY),format!("{}\n",json!({"schema":SCHEMA,"cache_key":"key","inventory_digest":blake3::hash(text.as_bytes()).to_hex().to_string()}))).unwrap();
            assert_eq!(
                verify(dir.path(), "key", true, &[], &runtime),
                expected,
                "variant {variant}"
            );
            assert_eq!(
                read_verified(dir.path(), "key", true, &[], &runtime).is_some(),
                expected,
                "full checker variant {variant}"
            );
        }
    }
    #[test]
    fn selected_objects_runtime_profile_key_and_abi_payload_are_bound() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("runtime.a");
        std::fs::write(&runtime, b"selected runtime").unwrap();
        std::fs::write(dir.path().join("core.o"), b"selected object").unwrap();
        std::fs::write(dir.path().join("core.poda"), b"selected summary").unwrap();
        let artifacts = vec!["core.o".into(), "core.poda".into()];
        let bundle =
            super::super::closure::audit(&[("sdk", "define i64 @body() {\n ret i64 1\n}\n")]);
        publish(dir.path(), "key", true, &artifacts, &runtime, &bundle).unwrap();
        assert!(verify(dir.path(), "key", true, &artifacts, &runtime));
        assert!(!verify(dir.path(), "another", true, &artifacts, &runtime));
        assert!(!verify(dir.path(), "key", false, &artifacts, &runtime));
        std::fs::write(&runtime, b"modified runtime").unwrap();
        assert!(!verify(dir.path(), "key", true, &artifacts, &runtime));
        std::fs::write(&runtime, b"selected runtime").unwrap();
        std::fs::write(dir.path().join("core.o"), b"modified object").unwrap();
        assert!(!verify(dir.path(), "key", true, &artifacts, &runtime));
        std::fs::write(dir.path().join("core.o"), b"selected object").unwrap();
        let file = dir.path().join(FILE);
        let mut json: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        json["inventory"]["functions"][0]["abi"]["result"] = json!("ptr");
        std::fs::write(&file, format!("{}\n", json)).unwrap();
        assert!(!verify(dir.path(), "key", true, &artifacts, &runtime));
    }
    #[test]
    fn absent_old_duplicate_or_traversing_publication_does_not_verify() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("runtime.a");
        std::fs::write(&runtime, b"runtime").unwrap();
        assert!(!verify(dir.path(), "key", true, &[], &runtime));
        let bundle = super::super::closure::audit(&[]);
        assert!(
            publish(
                dir.path(),
                "key",
                true,
                &["../escape".into()],
                &runtime,
                &bundle
            )
            .is_err()
        );
        std::fs::write(dir.path().join("core.o"), b"object").unwrap();
        assert!(
            publish(
                dir.path(),
                "key",
                true,
                &["core.o".into(), "core.o".into()],
                &runtime,
                &bundle
            )
            .is_err()
        );
        publish(dir.path(), "key", true, &[], &runtime, &bundle).unwrap();
        std::fs::write(dir.path().join(READY), "{\"schema\":0}").unwrap();
        assert!(!verify(dir.path(), "key", true, &[], &runtime));
    }
}
