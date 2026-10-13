//! Persistence of the engine state across processes (`docs/BUILD-MOTOR.md`
//! §4.1): the equivalent of `build_runner`'s `asset_graph.json`. A new
//! process reuses the actions whose queries still have the same digest,
//! instead of running them again. **Only when the user asks for it**
//! (`--estado`, [`ENV_VAR`]): governing rule 6 of `PLANO.md` forbids on-disk
//! bookkeeping that nobody asked for.
//!
//! The file stores, per action, the action key (phase, options, input and
//! expected outputs), the origin, the queries with their digests and the
//! digest of each output; the content of the outputs lives in `blobs/`,
//! addressed by blake3. Only what can be revalidated without the previous
//! process goes in:
//!
//! * file queries (`File`, `Exists`, `Glob`, `ActiveGlobs`) — a semantic
//!   query depends on the session's database, which a new process does not
//!   have in the same form (in a single pass it does not even answer);
//! * actions that do not belong to a per-package generator (its
//!   revalidation is the package round, which stays in memory);
//! * nothing pending or measured.
//!
//! The inputs the queries do not see go into the key: the engine version,
//! the executable's identity (the code of the native generators), the plan,
//! the lock versions and the mode (`--release`). For an action of the Dart
//! executor, the builder code — the bootstrap depfile's files outside the
//! `hosted` packages, by content.
use crate::query::{Query, Digest};
use crate::graph::AssetId;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Environment variable that turns on the on-disk state in `dev`, `serve`
/// and `compile-js` (and in `dartforge build`, besides `--estado`).
pub const ENV_VAR: &str = "DARTFORGE_BUILD_ESTADO";

/// Did the user ask for the on-disk state through the environment
/// (`DARTFORGE_BUILD_ESTADO` with a non-empty value other than `0`)? Without a
/// request nothing goes to disk: governing rule 6 of `PLANO.md` ("the disk
/// only receives what the user asks for").
pub fn requested_in_env() -> bool {
    std::env::var_os(ENV_VAR).is_some_and(|v| !v.is_empty() && v != "0")
}

/// File format; it changes when the content changes shape.
const FORMAT: u64 = 1;

/// A project's state directory.
pub fn directory(root_dir: &Path) -> PathBuf {
    root_dir
        .join(".dart_tool")
        .join("dartforge")
        .join("build")
        .join("estado")
}

/// Who produced the saved action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedOrigin {
    Native(String),
    Dart,
    Support,
    Skipped,
}

/// A saved action.
#[derive(Debug, Clone)]
pub struct SavedAction {
    pub key: String,
    pub origin: SavedOrigin,
    pub motivo: Option<String>,
    pub queries: Vec<(Query, Option<Digest>)>,
    pub outputs: Vec<(AssetId, Option<Digest>)>,
    pub from_support: Vec<AssetId>,
    pub deleted_outputs: Vec<AssetId>,
}

/// A project's saved state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Global key (engine, executable, plan, lock, mode).
    pub key: String,
    /// The code of the Dart builders: file and digest of its content. `None`:
    /// there is no saved Dart action.
    pub dart_code: Option<Vec<(PathBuf, Digest)>>,
    pub actions: Vec<SavedAction>,
}

/// Identity of the executable that runs the engine: the code of the native
/// generators is not in any query. Path, size and date of the file.
pub fn executable_identity() -> String {
    let Ok(exe) = std::env::current_exe() else {
        return "?".into();
    };
    let meta = std::fs::metadata(&exe).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let date = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}|{size}|{date}", exe.display())
}

fn hex(d: &Digest) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Digest> {
    if s.len() != 64 {
        return None;
    }
    let mut d = [0u8; 32];
    for (i, byte) in d.iter_mut().enumerate() {
        *byte = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(d)
}

fn path_str(p: &Path) -> Option<String> {
    p.to_str().map(str::to_string)
}

/// A query that can be persisted as JSON; `None` for the semantic ones.
fn query_json(c: &Query) -> Option<Value> {
    Some(match c {
        Query::File(p) => json!({"t": "arquivo", "p": path_str(p)?}),
        Query::Exists(p) => json!({"t": "existe", "p": path_str(p)?}),
        Query::Glob { dir, pattern } => {
            json!({"t": "glob", "p": path_str(dir)?, "padrao": pattern})
        }
        Query::ActiveGlobs {
            dir,
            pattern,
            candidatos,
        } => {
            json!({"t": "glob_ativos", "p": path_str(dir)?, "padrao": pattern, "candidatos": candidatos})
        }
        _ => return None,
    })
}

fn query_from_json(v: &Value) -> Option<Query> {
    let p = || v.get("p").and_then(Value::as_str).map(PathBuf::from);
    let pattern = || v.get("padrao").and_then(Value::as_str).map(str::to_string);
    Some(match v.get("t")?.as_str()? {
        "arquivo" => Query::File(p()?),
        "existe" => Query::Exists(p()?),
        "glob" => Query::Glob {
            dir: p()?,
            pattern: pattern()?,
        },
        "glob_ativos" => {
            let mut candidates = Vec::new();
            for c in v.get("candidatos")?.as_array()? {
                candidates.push((c.get(0)?.as_str()?.to_string(), c.get(1)?.as_bool()?));
            }
            Query::ActiveGlobs {
                dir: p()?,
                pattern: pattern()?,
                candidatos: candidates,
            }
        }
        _ => return None,
    })
}

/// Can the query be saved (is it a file query)?
pub fn persistable(c: &Query) -> bool {
    query_json(c).is_some()
}

fn ids(v: &[AssetId]) -> Value {
    Value::Array(v.iter().map(|id| Value::String(id.text())).collect())
}

fn ids_from(v: Option<&Value>) -> Option<Vec<AssetId>> {
    v?.as_array()?
        .iter()
        .map(|x| AssetId::from_text(x.as_str()?))
        .collect()
}

fn action_json(a: &SavedAction) -> Option<Value> {
    let origin = match &a.origin {
        SavedOrigin::Native(n) => json!({"nativo": n}),
        SavedOrigin::Dart => json!("dart"),
        SavedOrigin::Support => json!("apoio"),
        SavedOrigin::Skipped => json!("omitida"),
    };
    let mut queries = Vec::with_capacity(a.queries.len());
    for (c, d) in &a.queries {
        let mut v = query_json(c)?;
        v["d"] = d
            .as_ref()
            .map(|d| Value::String(hex(d)))
            .unwrap_or(Value::Null);
        queries.push(v);
    }
    let outputs: Vec<Value> = a
        .outputs
        .iter()
        .map(|(id, d)| json!([id.text(), d.as_ref().map(hex)]))
        .collect();
    Some(json!({
        "chave": a.key,
        "origem": origin,
        "motivo": a.motivo,
        "consultas": queries,
        "saidas": outputs,
        "do_apoio": ids(&a.from_support),
        "apagados": ids(&a.deleted_outputs),
    }))
}

fn action_from_json(v: &Value) -> Option<SavedAction> {
    let origin = match v.get("origem")? {
        Value::String(s) if s == "dart" => SavedOrigin::Dart,
        Value::String(s) if s == "apoio" => SavedOrigin::Support,
        Value::String(s) if s == "omitida" => SavedOrigin::Skipped,
        o => SavedOrigin::Native(o.get("nativo")?.as_str()?.to_string()),
    };
    let mut queries = Vec::new();
    for c in v.get("consultas")?.as_array()? {
        let d = match c.get("d")? {
            Value::Null => None,
            d => Some(from_hex(d.as_str()?)?),
        };
        queries.push((query_from_json(c)?, d));
    }
    let mut outputs = Vec::new();
    for s in v.get("saidas")?.as_array()? {
        let id = AssetId::from_text(s.get(0)?.as_str()?)?;
        let d = match s.get(1)? {
            Value::Null => None,
            d => Some(from_hex(d.as_str()?)?),
        };
        outputs.push((id, d));
    }
    Some(SavedAction {
        key: v.get("chave")?.as_str()?.to_string(),
        origin: origin,
        motivo: v.get("motivo").and_then(Value::as_str).map(str::to_string),
        queries: queries,
        outputs: outputs,
        from_support: ids_from(v.get("do_apoio"))?,
        deleted_outputs: ids_from(v.get("apagados"))?,
    })
}

/// Reads the saved state. A missing file, one of another format or a
/// corrupted one: `None` (the engine starts from scratch, as without state).
pub fn read(dir: &Path) -> Option<State> {
    let text = std::fs::read_to_string(dir.join("estado.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    if v.get("formato")?.as_u64()? != FORMAT {
        return None;
    }
    let dart_code = match v.get("codigo_dart")? {
        Value::Null => None,
        c => {
            let mut l = Vec::new();
            for pair in c.as_array()? {
                l.push((
                    PathBuf::from(pair.get(0)?.as_str()?),
                    from_hex(pair.get(1)?.as_str()?)?,
                ));
            }
            Some(l)
        }
    };
    let actions = v
        .get("acoes")?
        .as_array()?
        .iter()
        .map(action_from_json)
        .collect::<Option<Vec<_>>>()?;
    Some(State {
        key: v.get("chave")?.as_str()?.to_string(),
        dart_code: dart_code,
        actions: actions,
    })
}

/// Content of a saved output, checked against its digest.
pub fn ler_blob(dir: &Path, d: &Digest) -> Option<Arc<[u8]>> {
    let b = std::fs::read(dir.join("blobs").join(hex(d))).ok()?;
    (blake3::hash(&b).as_bytes() == d).then(|| Arc::from(b))
}

/// Writes the state and the new contents, and deletes the blobs that no
/// action references anymore. Writing the JSON is atomic (temporary file and
/// `rename`).
///
/// # Errors
///
/// Write failure in the state directory.
pub fn write_state(
    dir: &Path,
    state: &State,
    contents: &[(Digest, Arc<[u8]>)],
) -> Result<(), String> {
    let blobs = dir.join("blobs");
    std::fs::create_dir_all(&blobs).map_err(|e| format!("{}: {e}", blobs.display()))?;
    let mut live = std::collections::HashSet::new();
    for (d, b) in contents {
        let name = hex(d);
        let p = blobs.join(&name);
        if !p.is_file() {
            let tmp = blobs.join(format!("{name}.tmp"));
            std::fs::write(&tmp, &b[..]).map_err(|e| format!("{}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, &p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        live.insert(name);
    }
    let actions: Vec<Value> = state.actions.iter().filter_map(action_json).collect();
    let code = state.dart_code.as_ref().map(|l| {
        Value::Array(
            l.iter()
                .filter_map(|(p, d)| Some(json!([path_str(p)?, hex(d)])))
                .collect(),
        )
    });
    let v = json!({
        "formato": FORMAT,
        "chave": state.key,
        "codigo_dart": code,
        "acoes": actions,
    });
    let file = dir.join("estado.json");
    let tmp = dir.join("estado.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(&v).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &file).map_err(|e| format!("{}: {e}", file.display()))?;
    if let Ok(ls) = std::fs::read_dir(&blobs) {
        for e in ls.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !live.contains(&n) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trip_and_checked_blob() {
        let dir = tempfile::tempdir().unwrap();
        let content: Arc<[u8]> = Arc::from(&b"output"[..]);
        let d = *blake3::hash(&content).as_bytes();
        let action = SavedAction {
            key: "k".into(),
            origin: SavedOrigin::Native("sass_builder:sass_builder".into()),
            motivo: None,
            queries: vec![
                (Query::File("/p/lib/a.scss".into()), Some(d)),
                (Query::Exists("/p/lib/b.scss".into()), None),
                (
                    Query::ActiveGlobs {
                        dir: "/p".into(),
                        pattern: "lib/**".into(),
                        candidatos: vec![("lib/a.g.dart".into(), true)],
                    },
                    Some(d),
                ),
            ],
            outputs: vec![
                (AssetId::new("p", "lib/a.css"), Some(d)),
                (AssetId::new("p", "lib/a.css.map"), None),
            ],
            from_support: vec![],
            deleted_outputs: vec![AssetId::new("p", "lib/a.scss")],
        };
        let e = State {
            key: "c".into(),
            dart_code: Some(vec![("/p/tool/b.dart".into(), d)]),
            actions: vec![action],
        };
        write_state(dir.path(), &e, &[(d, content.clone())]).unwrap();
        let read = read(dir.path()).unwrap();
        assert_eq!(read.key, "c");
        assert_eq!(read.dart_code, e.dart_code);
        assert_eq!(read.actions[0].queries, e.actions[0].queries);
        assert_eq!(read.actions[0].outputs, e.actions[0].outputs);
        assert_eq!(read.actions[0].origin, e.actions[0].origin);
        assert_eq!(read.actions[0].deleted_outputs, e.actions[0].deleted_outputs);
        assert_eq!(ler_blob(dir.path(), &d).as_deref(), Some(&b"output"[..]));
        // A tampered blob is not accepted.
        std::fs::write(dir.path().join("blobs").join(hex(&d)), b"other").unwrap();
        assert!(ler_blob(dir.path(), &d).is_none());
        // A semantic query cannot be persisted.
        assert!(!persistable(&Query::ApiBiblioteca(
            "package:p/a.dart".into()
        )));
        // An unreferenced blob goes away on the next write.
        write_state(
            dir.path(),
            &State {
                key: "c".into(),
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert!(!dir.path().join("blobs").join(hex(&d)).exists());
    }
}
