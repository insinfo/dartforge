//! Small source patches of the DDC SDK for the production profile, which
//! compiles the SDK through our own pipeline (`docs/JS-PRODUCAO-TAMANHO.md`
//! §12).
//!
//! DDC compiles a few runtime functions with a `dynamic` receiver, so they go
//! through `dart.dload`/`dart.dsend`. Those helpers pull in the whole
//! `noSuchMethod` path (`InvocationImpl`, `Map.fromIterable`, the map
//! classes): in a closed world every program pays for it, `print('hi')`
//! included. Each patch here states the static type the code already has by
//! contract, without changing what it computes:
//!
//! * `diagnoseIndexError(indexable, index)` reads `indexable.length`. Every
//!   caller passes a JavaScript indexable (`JSArray`, `JSString`, a typed
//!   array, a DOM list), whose JavaScript `length` is the Dart one;
//! * `stringContainsUnchecked(receiver, other, startIndex)` receives the
//!   `Pattern` of `String.contains`;
//! * `JSNoSuchMethodError` (the extension of the JavaScript `TypeError`,
//!   live in every program) builds its message with four `RegExp`s and
//!   `String.split`, which kept `JSSyntaxRegExp`, `FormatException` and the
//!   match iterators. The same JavaScript regular expressions now run
//!   directly (`new RegExp(source).exec(message)`, which is what
//!   `firstMatch` does) and the stack is cut with the JavaScript `split`.
//!
//! The patched file replaces the original through the overlay of
//! [`SdkLayout`] (the content changes, the path stays), so the SDK cache key
//! covers it. A patch whose text is not found (another SDK) is skipped: the
//! original is still correct, only larger.

use dartforge_elements::sdk::SdkLayout;
use std::path::{Path, PathBuf};

/// (file under the SDK's `lib/`, original text, replacement).
const PATCHES: &[(&str, &str, &str)] = &[
    (
        "_internal/js_dev_runtime/private/js_helper.dart",
        "Error diagnoseIndexError(indexable, int index) {\n  int length = indexable.length;",
        "Error diagnoseIndexError(indexable, int index) {\n  int length = JS('int', '#.length', indexable);",
    ),
    (
        "_internal/js_dev_runtime/private/string_helper.dart",
        "    var substr = receiver.substring(startIndex);\n    return other.allMatches(substr).isNotEmpty;",
        "    var substr = receiver.substring(startIndex);\n    return (other as Pattern).allMatches(substr).isNotEmpty;",
    ),
    (
        "_internal/js_dev_runtime/private/interceptors.dart",
        r##"  static final _nullError = RegExp(r"^Cannot read property '(.+)' of null$");
  static final _notAFunction = RegExp(r"^(.+) is not a function$");
  static final _extensionName = RegExp(r"^Symbol\(dartx\.(.+)\)$");
  static final _privateName = RegExp(r"^Symbol\((_.+)\)$");

  String? _fieldName(String message) {
    RegExpMatch? match = _nullError.firstMatch(message);
    if (match == null) return null;
    String name = match[1]!;
    match = _extensionName.firstMatch(name) ?? _privateName.firstMatch(name);
    return match != null ? match[1] : name;
  }

  String? _functionCallTarget(String message) {
    var match = _notAFunction.firstMatch(message);
    return match != null ? match[1] : null;
  }

  String dartStack() {
    var stack = super.dartStack();
    // Strip TypeError from first line.
    stack = toString() + '\n' + stack.split('\n').sublist(1).join('\n');
    return stack;
  }"##,
        r##"  String? _fieldName(String message) {
    Object? match = JS('', 'new RegExp(#).exec(#)', r"^Cannot read property '(.+)' of null$", message);
    if (match == null) return null;
    String name = JS('String', '#[1]', match);
    match = JS('', 'new RegExp(#).exec(#)', r"^Symbol\(dartx\.(.+)\)$", name) ??
        JS('', 'new RegExp(#).exec(#)', r"^Symbol\((_.+)\)$", name);
    return match != null ? JS('String', '#[1]', match) : name;
  }

  String? _functionCallTarget(String message) {
    Object? match = JS('', 'new RegExp(#).exec(#)', r"^(.+) is not a function$", message);
    return match != null ? JS('String', '#[1]', match) : null;
  }

  String dartStack() {
    var stack = super.dartStack();
    // Strip TypeError from first line.
    stack = toString() + '\n' + JS<String>('!', r'#.split("\n").slice(1).join("\n")', stack);
    return stack;
  }"##,
    ),
];

/// Applies [`PATCHES`] to `sdk` as overlay replacements. The patched files
/// are written once, by content, under `dir`.
pub fn apply(sdk: &mut SdkLayout, dir: &Path) {
    let mut by_file: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
    for &(file, old, new) in PATCHES {
        match by_file.iter_mut().find(|(f, _)| *f == file) {
            Some((_, v)) => v.push((old, new)),
            None => by_file.push((file, vec![(old, new)])),
        }
    }
    for (file, edits) in by_file {
        let original = normalize(&sdk.root.join(file));
        if sdk.replacements.contains_key(&original) {
            continue;
        }
        let Ok(mut text) = std::fs::read_to_string(&original) else { continue };
        let mut changed = false;
        for (old, new) in edits {
            // CRLF checkouts of the SDK: the patch is written with `\n`.
            let crlf = text.contains("\r\n");
            let (old, new) = if crlf { (old.replace('\n', "\r\n"), new.replace('\n', "\r\n")) } else { (old.to_string(), new.to_string()) };
            if text.matches(&old).count() == 1 {
                text = text.replacen(&old, &new, 1);
                changed = true;
            }
        }
        if !changed {
            continue;
        }
        if let Some(path) = write_by_content(dir, file, &text) {
            sdk.replacements.insert(original, path);
        }
    }
}

/// Resolves `.` and `..` lexically, like the key of the overlay
/// (`SdkLayout::replacement`).
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Writes `text` to `dir/<hash>/<file>` unless it is already there, and
/// returns the path. Concurrent compilations write the same bytes; the
/// rename makes each write atomic.
fn write_by_content(dir: &Path, file: &str, text: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    let path = dir.join(format!("{:016x}", h.finish())).join(file);
    if std::fs::read_to_string(&path).is_ok_and(|t| t == text) {
        return Some(path);
    }
    std::fs::create_dir_all(path.parent()?).ok()?;
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, text).ok()?;
    std::fs::rename(&tmp, &path).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::PATCHES;

    #[test]
    fn every_patch_changes_its_text() {
        for (file, old, new) in PATCHES {
            assert_ne!(old, new, "{file}");
            assert!(!old.is_empty(), "{file}");
        }
    }
}
