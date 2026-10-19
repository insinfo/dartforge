//! `sass_builder:sass_builder` through the Sass of `ng_generator`
//! (`sass::compile_asset`, public API), which is dart-sass 1.102.0 byte for
//! byte (the `dartforge-sass` crate) or, when `pubspec.lock` brings `sass`
//! 1.66.0, the compatibility mode for it (`DartSassVersion::V1_66_0`).
//!
//! Both styles (`compressed` and `expanded`, the `sass_builder` default) and
//! the `.css.map` when `sourceMaps` is on (`dev` turns it on by default), with
//! the `sourceMappingURL` comment in the `.css`, as `SassBuilder` writes them.
//! The stylesheets of `@use`/`@forward`/`@import` resolve as in
//! `BuildImporter` (relative to the file, and `package:` through the package
//! roots); every file read or probed is a dependency of the action.
use crate::query::Query;
use crate::executor::{GeneratorCtx, NativeGenerator, NativeRequest, NativeOutput};
use crate::value::Value;
use dartforge_ng_generator::sass::{AssetOptions, Style, AssetReader, DartSassVersion, compile_asset};
use std::path::Path;
use std::sync::Arc;

/// The `BuildStep` for Sass: what the engine has in memory (outputs of
/// previous builders) or the disk.
struct StepReader<'a> {
    memory: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
}

impl AssetReader for StepReader<'_> {
    fn exists(&self, path: &Path) -> bool {
        (self.memory)(&dartforge_elements::generated::key(path)).is_some() || path.is_file()
    }

    fn read(&self, path: &Path) -> std::io::Result<String> {
        match (self.memory)(&dartforge_elements::generated::key(path)) {
            Some(b) => String::from_utf8(b.to_vec())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            None => std::fs::read_to_string(path),
        }
    }
}

pub struct SassNative;

impl NativeGenerator for SassNative {
    fn key(&self) -> &'static str {
        "sass_builder:sass_builder"
    }

    fn covers(&self, _factory: &str) -> bool {
        true
    }

    fn by_package(&self) -> bool {
        false
    }

    fn verified(&self) -> bool {
        true
    }

    fn generate(
        &self,
        ctx: &mut GeneratorCtx<'_>,
        request: &NativeRequest,
    ) -> Result<NativeOutput, String> {
        let mut s = NativeOutput::default();
        // The dart-sass from the lock (the engine only calls with an imitated
        // version).
        let version = request.versions.get("sass").map(String::as_str);
        let Some(version) = version.and_then(DartSassVersion::from_lock) else {
            return Err(format!(
                "sass: dart-sass version in the lock is not imitated: {}",
                version.unwrap_or("missing")
            ));
        };
        for a in &request.actions {
            let name = a.input.path.rsplit('/').next().unwrap_or_default();
            if name.starts_with('_') {
                // Partial: the official builder generates nothing
                // (`sass_builder.dart:43`).
                continue;
            }
            // A missing `outputStyle` is `sass_builder`'s `expanded`; for an
            // unknown value it warns and uses `expanded` — we do not have the
            // warning.
            let style = match a.options.get("outputStyle") {
                None | Some(Value::Null) => Style::Expanded,
                Some(Value::Text(v)) if v == "expanded" => Style::Expanded,
                Some(Value::Text(v)) if v == "compressed" => Style::Compressed,
                Some(_) => {
                    s.refusals.insert(
                        a.natural_input.clone(),
                        "sass: unknown outputStyle (the official builder warns)".into(),
                    );
                    continue;
                }
            };
            let Some(source) = ctx.read(&a.natural_input) else {
                s.refusals
                    .insert(a.natural_input.clone(), "sass: unreadable input".into());
                continue;
            };
            let text = String::from_utf8_lossy(&source);
            let source_map = a.options.get("sourceMaps") == Some(&Value::Bool(true));
            let reader = StepReader {
                memory: ctx.memory,
            };
            let sheet = match compile_asset(
                &text,
                &a.input.package,
                &a.input.path,
                &request.roots,
                &reader,
                AssetOptions { style, source_map, version },
            ) {
                Ok(f) => f,
                Err(m) => {
                    let first = m.lines().next().unwrap_or_default().to_owned();
                    s.refusals.insert(
                        a.natural_input.clone(),
                        format!("sass: dart-sass rejects it: {first}"),
                    );
                    continue;
                }
            };
            for file in sheet.read_files.iter().chain(&sheet.probed_files) {
                ctx.register(Query::File(dartforge_elements::generated::key(
                    file,
                )));
            }
            if let Some((_, n)) = a.outputs.iter().find(|(id, _)| id.path.ends_with(".css")) {
                s.outputs.insert(n.clone(), sheet.css.into_bytes());
            }
            if let Some(m) = sheet.map
                && let Some((_, n)) = a
                    .outputs
                    .iter()
                    .find(|(id, _)| id.path.ends_with(".css.map"))
            {
                s.outputs.insert(n.clone(), m.into_bytes());
            }
        }
        Ok(s)
    }
}
