//! SDK layout: `libraries.json` says where each `dart:x` lives and which
//! patches complete it.
//!
//! The file is the same one the CFE reads. The chosen section is
//! `dartdevc`, because emission follows the DDC model
//! (docs/FRONTEND-ARQUITETURA.md §5); the `dart2js` section has the same
//! source libraries with different patches, and switching models means
//! switching sections.
use dartforge_frontend::{Feature, LanguageVersion};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A `dart:` library of the SDK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkLibrary {
    /// Name after `dart:` (`core`, `_js_helper`).
    pub name: String,
    /// File of the source library.
    pub path: PathBuf,
    /// Patch files, in the order the SDK applies them.
    pub patches: Vec<PathBuf>,
    /// `supported: false` in `libraries.json` (`dart:io` in DDC): importable,
    /// but every use throws `UnsupportedError`.
    pub supported: bool,
}

/// Map `dart:x` → files, for one `libraries.json` target.
#[derive(Debug, Clone, Default)]
pub struct SdkLayout {
    /// The SDK's `lib/` directory.
    pub root: PathBuf,
    pub libraries: HashMap<String, SdkLibrary>,
    /// Explicit declared variables for conditional imports/exports. `None`
    /// retains compiler-profile library support; `Some(empty)` declares none,
    /// as the analyzer does. An absent variable never equals the string false.
    pub conditional_import_variables: Option<HashMap<String, String>>,
    /// The tool's **current** language version: that of a library without a
    /// marker and outside a package, and the ceiling of the markers
    /// (`docs/VERSOES-LINGUAGEM.md`, D2). Default 3.13; the CLI changes it with
    /// `--language-version`. The `dart:*` libraries always stay at the floor
    /// ([`LanguageVersion::FLOOR`]), because they are those of SDK 3.6.2 (D1).
    pub current_version: LanguageVersion,
    /// `--enable-experiment=…`: they only apply to libraries without a marker
    /// whose default version is the current one.
    pub experiments: Vec<Feature>,
    /// Overlay (`load_with_overlay`): SDK file, by normalized path → the
    /// file that replaces it. Empty without an overlay.
    pub replacements: HashMap<PathBuf, PathBuf>,
    /// SDK libraries a library outside the SDK may not import on this target,
    /// with the compiler's message (the overlay's `user_import_errors`:
    /// dart2wasm refuses `dart:ffi` in user code).
    pub user_import_errors: HashMap<String, String>,
    /// A library outside the SDK may not import the platform's private
    /// libraries (`dart:_internal`…), as the CFE refuses them: on with an overlay
    /// (the native and WebAssembly targets), off for the entries that load the
    /// SDK's own libraries.
    pub refuses_private_imports: bool,
}

/// Command-line language options: `--language-version x.y` (the current
/// version, D2) and `--enable-experiment=a,b`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageOptions {
    pub current_version: Option<LanguageVersion>,
    pub experiments: Vec<Feature>,
}

impl LanguageOptions {
    /// Consumes `arg` (and the next value, in `rest`) if it is a language
    /// option. `Ok(false)` when `arg` is not one.
    ///
    /// # Errors
    /// A version not in the `x.y` format, above the current one or below 2.12,
    /// and an unknown experiment.
    pub fn read_option<'a>(
        &mut self,
        arg: &str,
        rest: &mut impl Iterator<Item = &'a str>,
    ) -> Result<bool, String> {
        if arg == "--language-version" || arg.starts_with("--language-version=") {
            let value = match arg.split_once('=') {
                Some((_, v)) => v.to_string(),
                None => rest
                    .next()
                    .ok_or("--language-version requires x.y")?
                    .to_string(),
            };
            let v = LanguageVersion::parse(&value)
                .ok_or_else(|| format!("--language-version: '{value}' is not x.y"))?;
            if v > LanguageVersion::CURRENT || v < LanguageVersion::MINIMUM {
                return Err(format!(
                    "--language-version {v}: outside the supported range ({} to {})",
                    LanguageVersion::MINIMUM,
                    LanguageVersion::CURRENT
                ));
            }
            self.current_version = Some(v);
            return Ok(true);
        }
        if arg == "--enable-experiment" || arg.starts_with("--enable-experiment=") {
            let value = match arg.split_once('=') {
                Some((_, v)) => v.to_string(),
                None => rest
                    .next()
                    .ok_or("--enable-experiment requires names")?
                    .to_string(),
            };
            for name_ in value.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                let f = Feature::from_name(name_)
                    .ok_or_else(|| format!("--enable-experiment: unknown experiment '{name_}'"))?;
                if !self.experiments.contains(&f) {
                    self.experiments.push(f);
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Applies to the layout (what was not requested stays as it is).
    pub fn apply(&self, sdk: &mut SdkLayout) {
        if let Some(v) = self.current_version {
            sdk.current_version = v;
        }
        for f in &self.experiments {
            if !sdk.experiments.contains(f) {
                sdk.experiments.push(*f);
            }
        }
    }
}

impl SdkLayout {
    /// Reads `<lib>/libraries.json` and the `target` section (`dartdevc` or
    /// `dart2js`).
    ///
    /// # Errors
    /// Missing file, invalid JSON or nonexistent section.
    ///
    /// ```no_run
    /// let sdk = dartforge_elements::sdk::SdkLayout::load(std::path::Path::new("C:/tools/dartsdk-3.6.2/lib"), "dartdevc").unwrap();
    /// assert!(sdk.library("core").is_some());
    /// ```
    pub fn load(lib_dir: &Path, target: &str) -> Result<Self, String> {
        let path = lib_dir.join("libraries.json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("{} is invalid: {e}", path.display()))?;
        let mut libraries = HashMap::new();
        Self::read_section(&json, lib_dir, target, &path, &mut libraries, 0)?;
        Ok(SdkLayout {
            root: lib_dir.to_path_buf(),
            libraries,
            conditional_import_variables: None,
            current_version: LanguageVersion::CURRENT,
            experiments: Vec::new(),
            replacements: HashMap::new(),
            user_import_errors: HashMap::new(),
            refuses_private_imports: false,
        })
    }

    /// The SDK as the analyzer sees it (`FolderBasedDartSdk`): each library of
    /// `_internal/sdk_library_metadata/lib/libraries.dart` by the path declared
    /// in its `LibraryInfo`, **without patches** (the analyzer does not apply
    /// them: `external` declarations stay as they are, and the internal
    /// libraries are dart2js's, `_internal/js_runtime`). Without that file (a
    /// reduced SDK), the `dartdevc` profile of `libraries.json`.
    ///
    /// # Errors
    /// Those of [`SdkLayout::load`], when it falls back to it.
    pub fn load_as_analyzer(lib_dir: &Path) -> Result<Self, String> {
        let metadata = lib_dir.join("_internal/sdk_library_metadata/lib/libraries.dart");
        let Ok(text) = std::fs::read_to_string(&metadata) else {
            return Self::load(lib_dir, "dartdevc");
        };
        let mut libraries = HashMap::new();
        // `'name': const LibraryInfo(\n    'path',` (single or double quotes).
        let bytes = text.as_bytes();
        let mut i = 0usize;
        while let Some(rel) = text[i..].find("const LibraryInfo(") {
            let pos = i + rel;
            i = pos + "const LibraryInfo(".len();
            // The name: the last string before the `:` that precedes the `const`.
            let before = text[..pos].trim_end();
            let Some(before) = before.strip_suffix(':') else {
                continue;
            };
            let before = before.trim_end();
            let Some(quote) = before.chars().last().filter(|c| *c == '\'' || *c == '"') else {
                continue;
            };
            let body_node = &before[..before.len() - 1];
            let Some(ini) = body_node.rfind(quote) else {
                continue;
            };
            let name_ = body_node[ini + 1..].to_string();
            // The path: the first string after the `(`.
            let mut j = i;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            let Some(&q) = bytes.get(j) else { break };
            if q != b'\'' && q != b'"' {
                continue;
            }
            let Some(end) = text[j + 1..].find(q as char) else {
                break;
            };
            let mut path = &text[j + 1..j + 1 + end];
            // `dart2jsPath:` replaces the path (the `SdkLibrariesReader`'s
            // `_LibraryBuilder` writes the named argument over the positional one):
            // `dart:html_common` is `html_common_dart2js.dart`.
            if let Some(closes) = text[j..].find(')') {
                let args = &text[j..j + closes];
                if let Some(k) = args.find("dart2jsPath:") {
                    let rest = args[k + "dart2jsPath:".len()..].trim_start();
                    if let Some(q2) = rest.chars().next().filter(|c| *c == '\'' || *c == '"')
                        && let Some(f2) = rest[1..].find(q2)
                    {
                        path = &rest[1..1 + f2];
                    }
                }
            }
            libraries.insert(
                name_.clone(),
                SdkLibrary {
                    name: name_,
                    path: lib_dir.join(path),
                    patches: Vec::new(),
                    supported: true,
                },
            );
        }
        if libraries.is_empty() {
            return Self::load(lib_dir, "dartdevc");
        }
        Ok(SdkLayout {
            root: lib_dir.to_path_buf(),
            libraries,
            conditional_import_variables: None,
            current_version: LanguageVersion::CURRENT,
            experiments: Vec::new(),
            replacements: HashMap::new(),
            user_import_errors: HashMap::new(),
            refuses_private_imports: false,
        })
    }

    /// The SDK's `base` section with files swapped by the overlay at
    /// `overlay_dir` (the native backend's `native_sdk/`,
    /// docs/NATIVO-PLANO.md §7, P5a).
    ///
    /// `<overlay_dir>/libraries.json` has a `target` section with `base` (the
    /// starting SDK section, `vm`) and `replaces`: a path relative to the SDK's
    /// `lib/` → a file relative to the overlay. The swap is by **content**: the
    /// loaded unit keeps the SDK path, so a `part` of a swapped file resolves
    /// next to the original (and may be swapped too). That way the overlay only
    /// swaps what depends on the VM's internals without copying the rest of the
    /// patch.
    ///
    /// # Errors
    /// Those of [`SdkLayout::load`], a missing section, and a file (original
    /// or substitute) that does not exist — a swap that silently does not
    /// happen is an SDK different from the declared one.
    pub fn load_with_overlay(
        lib_dir: &Path,
        overlay_dir: &Path,
        target: &str,
    ) -> Result<Self, String> {
        let path = overlay_dir.join("libraries.json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("{} is invalid: {e}", path.display()))?;
        let section_ = json
            .get(target)
            .ok_or_else(|| format!("section {target} missing in {}", path.display()))?;
        let base = section_
            .get("base")
            .and_then(|b| b.as_str())
            .ok_or_else(|| format!("{target}.base missing in {}", path.display()))?;
        let mut sdk = Self::load(lib_dir, base)?;
        sdk.refuses_private_imports = true;
        // A target may change whole library entries of its base (another set
        // of patches, `supported: false`), add libraries its platform has and
        // the base does not (`"new": true`, the WebAssembly target's JavaScript
        // interop) and drop libraries its platform does not have. Paths are
        // relative to the SDK's `lib/`; `overlay:<path>` names a file of the
        // overlay directory.
        if let Some(entries) = section_.get("libraries").and_then(|l| l.as_object()) {
            for (name, entry) in entries {
                let adds = entry.get("new").and_then(|n| n.as_bool()).unwrap_or(false);
                if sdk.libraries.contains_key(name) == adds {
                    let problem = if adds {
                        "is marked new, but the base already has it"
                    } else {
                        "changes a library the base does not have"
                    };
                    return Err(format!(
                        "{target}.libraries[{name}] {problem} ({base}, {})",
                        path.display()
                    ));
                }
                let library = Self::library_entry_in(name, entry, lib_dir, Some(overlay_dir))?;
                for file in std::iter::once(&library.path).chain(&library.patches) {
                    if !file.is_file() {
                        return Err(format!(
                            "{target}.libraries[{name}] points to {}, which does not exist",
                            file.display()
                        ));
                    }
                }
                sdk.libraries.insert(name.clone(), library);
            }
        }
        if let Some(errors) = section_
            .get("user_import_errors")
            .and_then(|e| e.as_object())
        {
            for (name, message) in errors {
                let message = message.as_str().ok_or_else(|| {
                    format!(
                        "{target}.user_import_errors[{name}] is not a string in {}",
                        path.display()
                    )
                })?;
                sdk.user_import_errors
                    .insert(name.clone(), message.to_string());
            }
        }
        if let Some(names) = section_.get("remove").and_then(|r| r.as_array()) {
            for name in names {
                let name = name.as_str().ok_or_else(|| {
                    format!("{target}.remove has a non-string in {}", path.display())
                })?;
                if sdk.libraries.remove(name).is_none() {
                    return Err(format!(
                        "{target}.remove names {name}, which the base {base} does not have ({})",
                        path.display()
                    ));
                }
            }
        }
        if let Some(swaps) = section_.get("replaces").and_then(|s| s.as_object()) {
            for (original, new_) in swaps {
                let new_ = new_.as_str().ok_or_else(|| {
                    format!(
                        "{target}.replaces[{original}] is not a string in {}",
                        path.display()
                    )
                })?;
                let original = crate::load::normalize(&lib_dir.join(original));
                let new_ = crate::load::normalize(&overlay_dir.join(new_));
                if !original.is_file() {
                    return Err(format!(
                        "the overlay replaces {}, which does not exist in the SDK",
                        original.display()
                    ));
                }
                if !new_.is_file() {
                    return Err(format!(
                        "the overlay points to {}, which does not exist",
                        new_.display()
                    ));
                }
                sdk.replacements.insert(original, new_);
            }
        }
        Ok(sdk)
    }

    /// The file to read in place of `path`, if the overlay swaps it.
    pub fn replacement(&self, path: &Path) -> Option<&Path> {
        if self.replacements.is_empty() {
            return None;
        }
        self.replacements
            .get(&crate::load::normalize(path))
            .map(PathBuf::as_path)
    }

    /// Reads a section and, before it, the ones it includes (`"include":
    /// [{"target": "vm_common"}]`). The SDK 3.6.2 `vm` section only declares
    /// `cli`: `core`, `async`, `collection`… are in `vm_common`. Without
    /// following the `include`, the native backend loaded the program
    /// **without `dart:core`** — and every static type (`int`, `String`…)
    /// became `dynamic`. The section's own libraries override the included
    /// ones, as in the SDK's `libraries.yaml`.
    fn read_section(
        json: &serde_json::Value,
        lib_dir: &Path,
        target: &str,
        path: &Path,
        libraries: &mut HashMap<String, SdkLibrary>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 8 {
            return Err(format!(
                "cyclic include in section {target} of {}",
                path.display()
            ));
        }
        let section_ = json
            .get(target)
            .ok_or_else(|| format!("section {target} missing in {}", path.display()))?;
        if let Some(includes) = section_.get("include").and_then(|i| i.as_array()) {
            for inc in includes {
                if let Some(t) = inc.get("target").and_then(|t| t.as_str()) {
                    Self::read_section(json, lib_dir, t, path, libraries, depth + 1)?;
                }
            }
        }
        let section = section_
            .get("libraries")
            .and_then(|l| l.as_object())
            .ok_or_else(|| format!("section {target}.libraries missing in {}", path.display()))?;
        for (name, entry) in section {
            libraries.insert(name.clone(), Self::library_entry(name, entry, lib_dir)?);
        }
        Ok(())
    }

    /// One `libraries.json` entry: `uri`, `patches` (one or many) and
    /// `supported`, the paths relative to the SDK's `lib/`.
    fn library_entry(
        name: &str,
        entry: &serde_json::Value,
        lib_dir: &Path,
    ) -> Result<SdkLibrary, String> {
        Self::library_entry_in(name, entry, lib_dir, None)
    }

    /// [`Self::library_entry`] of an overlay target: a path written as
    /// `overlay:<path>` is a file of the overlay directory (a library or patch
    /// the SDK does not have, like the WebAssembly target's JavaScript interop),
    /// the others stay relative to the SDK's `lib/`.
    fn library_entry_in(
        name: &str,
        entry: &serde_json::Value,
        lib_dir: &Path,
        overlay_dir: Option<&Path>,
    ) -> Result<SdkLibrary, String> {
        let resolve = |p: &str| match (p.strip_prefix("overlay:"), overlay_dir) {
            (Some(rest), Some(dir)) => dir.join(rest),
            _ => lib_dir.join(p),
        };
        let uri = entry
            .get("uri")
            .and_then(|u| u.as_str())
            .ok_or_else(|| format!("library {name} without uri"))?;
        let patches = match entry.get("patches") {
            None => Vec::new(),
            Some(serde_json::Value::String(one)) => vec![resolve(one)],
            Some(serde_json::Value::Array(many)) => many
                .iter()
                .filter_map(|p| p.as_str())
                .map(resolve)
                .collect(),
            Some(_) => return Err(format!("library {name} with invalid patches")),
        };
        let supported = entry
            .get("supported")
            .and_then(|s| s.as_bool())
            .unwrap_or(true);
        Ok(SdkLibrary {
            name: name.to_string(),
            path: resolve(uri),
            patches,
            supported,
        })
    }

    pub fn library(&self, name: &str) -> Option<&SdkLibrary> {
        self.libraries.get(name)
    }

    /// Finds the SDK's `lib/`: `DARTFORGE_SDK_LIB`, the distribution's
    /// (`distribution::dart_sdk`), `DART_SDK/lib`, or the `dart` on the
    /// `PATH` (`<bin>/../lib`).
    pub fn discover() -> Option<PathBuf> {
        if let Ok(explicit) = std::env::var("DARTFORGE_SDK_LIB") {
            let path = PathBuf::from(explicit);
            if path.join("libraries.json").exists() {
                return Some(path);
            }
        }
        // The SDK the distribution carries (the version the native overlay knows),
        // before any SDK on the machine.
        if let Some(path) = crate::distribution::dart_sdk() {
            return Some(path);
        }
        if let Ok(sdk) = std::env::var("DART_SDK") {
            let path = PathBuf::from(sdk).join("lib");
            if path.join("libraries.json").exists() {
                return Some(path);
            }
        }
        let path_var = std::env::var_os("PATH")?;
        for dir in std::env::split_paths(&path_var) {
            for exe in ["dart.exe", "dart"] {
                if dir.join(exe).exists() {
                    // `<bin>/../lib` without the `..` (SDK paths show up in messages, as in the
                    // analyzer: `…/dart-sdk/lib/…`); with a symlinked `dart` (Homebrew), via the
                    // real executable.
                    let candidates = [
                        dir.parent().map(|p| p.join("lib")),
                        Some(dir.join("..").join("lib")),
                        std::fs::canonicalize(dir.join(exe))
                            .ok()
                            .and_then(|e| Some(e.parent()?.parent()?.join("lib"))),
                    ];
                    if let Some(lib) = candidates
                        .into_iter()
                        .flatten()
                        .find(|l| l.join("libraries.json").exists())
                    {
                        return Some(lib);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_dartdevc_section_of_local_sdk() {
        let Some(lib) = SdkLayout::discover() else {
            eprintln!("SDK not found; test skipped");
            return;
        };
        let sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
        let core = sdk.library("core").unwrap();
        assert!(core.path.ends_with("core/core.dart"));
        assert_eq!(core.patches.len(), 2);
        assert!(!sdk.library("io").unwrap().supported);
        assert!(sdk.library("_runtime").is_some());
    }

    /// The native overlay (`native_sdk/`, P5a) swaps by content the patches
    /// that depend on the VM's internals, and the program loads without
    /// diagnostics: the `part`s of the swapped patch resolve next to the
    /// original (and are swapped too).
    #[test]
    fn native_overlay_swaps_patches_and_loads() {
        let Some(lib) = SdkLayout::discover() else {
            eprintln!("SDK not found; test skipped");
            return;
        };
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native_sdk");
        let sdk = SdkLayout::load_with_overlay(&lib, &dir, "dartforge_native").unwrap();
        // One substitution per entry of `replaces` in libraries.json.
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("libraries.json")).unwrap())
                .unwrap();
        let expected_ones = json["dartforge_native"]["replaces"]
            .as_object()
            .unwrap()
            .len();
        assert_eq!(sdk.replacements.len(), expected_ones);
        assert!(expected_ones >= 20);
        let async_patch = &sdk.library("async").unwrap().patches[0];
        let new_ = sdk.replacement(async_patch).expect("async_patch swapped");
        assert!(
            new_.ends_with("async_patch.dart") && new_.starts_with(crate::load::normalize(&dir))
        );
        assert!(
            sdk.replacement(&sdk.library("core").unwrap().path)
                .is_none()
        );

        let tmp = tempfile::tempdir().unwrap();
        let entry_path = tmp.path().join("main.dart");
        std::fs::write(
            &entry_path,
            "import 'dart:async';\nvoid main() { Timer(Duration.zero, () {}); }\n",
        )
        .unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let (program, diags) = crate::load::load_lenient(&entry_path, &sdk, None, &mut interner);
        assert!(diags.is_empty(), "{diags:?}");
        let async_sources: Vec<&str> = program
            .libraries
            .iter()
            .find(|l| l.uri == "dart:async")
            .unwrap()
            .units
            .iter()
            .map(|u| program.units[u.0 as usize].source.as_str())
            .collect();
        for mark in [
            "DartForge_scheduleImmediate",
            "DartForge_Timer_new",
            "_wrapBody",
        ] {
            assert!(
                async_sources.iter().any(|s| s.contains(mark)),
                "{mark} missing from the loaded dart:async"
            );
        }
        assert!(
            !async_sources
                .iter()
                .any(|s| s.contains("class _SuspendState")),
            "the VM's _SuspendState is still loaded"
        );
    }

    /// The `vm` section only has `cli`; `core` comes from `vm_common`'s
    /// `include`.
    #[test]
    fn vm_section_follows_include() {
        let Some(lib) = SdkLayout::discover() else {
            eprintln!("SDK not found; test skipped");
            return;
        };
        let sdk = SdkLayout::load(&lib, "vm").unwrap();
        assert!(sdk.library("cli").is_some());
        let core = sdk.library("core").expect("core from vm_common");
        assert!(core.path.ends_with("core/core.dart"));
        assert!(!core.patches.is_empty());
    }
}
