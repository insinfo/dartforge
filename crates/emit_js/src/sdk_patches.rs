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

/// (switch, file under the SDK's `lib/`, original text, replacement). The
/// switch is an environment variable that, set to `0`, leaves the patch out
/// (`DARTFORGE_JSPROD_SDK_PATCHES=0` leaves them all out).
const PATCHES: &[(Option<&str>, &str, &str, &str)] = &[
    (
        None,
        "_internal/js_dev_runtime/private/js_helper.dart",
        "Error diagnoseIndexError(indexable, int index) {\n  int length = indexable.length;",
        "Error diagnoseIndexError(indexable, int index) {\n  int length = JS('int', '#.length', indexable);",
    ),
    (
        None,
        "_internal/js_dev_runtime/private/string_helper.dart",
        "    var substr = receiver.substring(startIndex);\n    return other.allMatches(substr).isNotEmpty;",
        "    var substr = receiver.substring(startIndex);\n    return (other as Pattern).allMatches(substr).isNotEmpty;",
    ),
    (
        None,
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
    // `getThrown` (live in every program with a `catch`) recognizes a stack
    // overflow by `message.contains('call stack')`: `String.contains` takes a
    // `Pattern`, and its generic path kept `allMatches` with the match
    // iterables, `Iterable` and `StringBuffer`. The argument is a literal
    // string: the JavaScript `includes` is the same test.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/errors.dart",
        "          error) &&\n      message.contains('call stack')) {",
        "          error) &&\n      JS<bool>('!', '#.includes(#)', message, 'call stack')) {",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/errors.dart",
        "          error) &&\n      message.contains('too much recursion')) {",
        "          error) &&\n      JS<bool>('!', '#.includes(#)', message, 'too much recursion')) {",
    ),
    // `getReifiedType` (behind `Object.toString` and `runtimeType`) calls
    // `getRtiForRecord` for a `RecordImpl`, which keeps `_Universe.bind`, the
    // binding rtis and `StringBuffer` in programs without records. As a
    // getter of `RecordImpl`, the computation lives only where a record is
    // created.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/rtti.dart",
        "      if (_jsInstanceOf(obj, RecordImpl)) return getRtiForRecord(obj);",
        "      if (_jsInstanceOf(obj, RecordImpl)) {\n        return JS<RecordImpl>('!', '#', obj)._reifiedRti;\n      }",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/records.dart",
        "  @override\n  String toString() => _toString(false);\n",
        "  @override\n  String toString() => _toString(false);\n\n  /// The rti of this record, for `getReifiedType`.\n  Object get _reifiedRti => getRtiForRecord(this);\n",
    ),
    // `_rtiToString` (behind `Object.toString` and every type error) cut the
    // library prefix of an interface name with `String.indexOf`, whose
    // `Pattern` path kept `JSSyntaxRegExp`, `matchAsPrefix` and `StringMatch`
    // alive. The separator is a `String`: `_Utils.stringIndexOf` is the same
    // JavaScript `indexOf`.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_shared/lib/rti.dart",
        "      name = name.substring(name.indexOf(Recipe.librarySeparatorString) + 1);",
        "      name = name.substring(\n          _Utils.stringIndexOf(name, Recipe.librarySeparatorString, 0) + 1);",
    ),
    // `trackCall` (behind every `dload`/`dput`/`dsend`) samples dynamic
    // calls for the DDC debugger only when `dart.__trackProfile` is set, which
    // only the development loader's settings do: in the single production
    // file it is never set, and the sampler kept `_CallMethodRecord`, a list
    // and `getReifiedType` alive.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/profile.dart",
        "trackCall(obj) {\n  if (JS<bool>('!', '!#', _trackProfile)) return;\n  int index = -1;\n  _totalCallRecords++;\n  if (_callMethodRecords.length == _callRecordSampleSize) {\n    // Ensure that each sample has an equal\n    // _callRecordSampleSize / _totalCallRecords chance of inclusion\n    // by choosing to include the new record in the sample the with the\n    // appropriate probability randomly evicting one of the existing records.\n    // Unfortunately we can't use the excellent Random.nextInt method defined\n    // by Dart from within this library.\n    index = JS<int>('!', 'Math.floor(Math.random() * #)', _totalCallRecords);\n    if (index >= _callMethodRecords.length) return; // don't sample\n  }\n  var record =\n      _CallMethodRecord(JS('', 'new Error()'), dart.getReifiedType(obj));\n  if (index == -1) {\n    _callMethodRecords.add(record);\n  } else {\n    _callMethodRecords[index] = record;\n  }\n}\n",
        "trackCall(obj) {}\n",
    ),
    // `InvocationImpl` (every failed dynamic call) built its named arguments
    // with `Map.unmodifiable(Map.fromIterable(…))`: both go through the
    // `LinkedHashMap` factory, which keeps `CustomHashMap`, `IdentityMap` and
    // the `fromIterable` filling alive. A literal map wrapped in the same
    // `UnmodifiableMapView` has the same keys, order and type.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "    return Map.unmodifiable(Map.fromIterable(getOwnPropertyNames(namedArgs),\n        key: _dartSymbol, value: (k) => JS('', '#[#]', namedArgs, k)));",
        "    var map = <Symbol, dynamic>{};\n    for (var k in getOwnPropertyNames(namedArgs)) {\n      map[_dartSymbol(k)] = JS('', '#[#]', namedArgs, k);\n    }\n    return UnmodifiableMapView<Symbol, dynamic>(map);",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/runtime.dart",
        "import 'dart:async';\n",
        "import 'dart:async';\nimport 'dart:collection' show UnmodifiableMapView;\n",
    ),
    // `bind` takes the tear-off's type from the class's signature table. A
    // tear-off whose type is the same for every method of that name gets it
    // from the call (`dart.tbind(o, "m", T)`, `emit_js`'s typed bind), and
    // the name needs no signature entry.
    (
        Some("DARTFORGE_JSPROD_TYPED_BIND"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "bind(obj, name, method) {\n  if (obj == null) obj = jsNull;",
        "bind(obj, name, method, [providedType]) {\n  if (obj == null) obj = jsNull;",
    ),
    (
        Some("DARTFORGE_JSPROD_TYPED_BIND"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "  var methodType = getMethodType(obj, name);\n  // Native JavaScript methods",
        "  var methodType = providedType ?? getMethodType(obj, name);\n  // Native JavaScript methods",
    ),
    (
        Some("DARTFORGE_JSPROD_TYPED_BIND"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "/// Binds the `call` method of an interface type, handling null.\n",
        "/// The tear-off of the method `name` of `obj`, of type `type`.\ntbind(obj, name, type) => bind(obj, name, null, type);\n\n/// Binds the `call` method of an interface type, handling null.\n",
    ),
];

/// Applies `PATCHES` to `sdk` as overlay replacements. The patched files
/// are written once, by content, under `dir`.
pub fn apply(sdk: &mut SdkLayout, dir: &Path) {
    let mut by_file: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
    for &(switch, file, old, new) in PATCHES {
        if switch.is_some_and(|v| std::env::var(v).is_ok_and(|v| v == "0")) {
            continue;
        }
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
        let Ok(text) = std::fs::read_to_string(&original) else {
            continue;
        };
        let Some(text) = apply_edits(&text, &edits) else {
            continue;
        };
        if let Some(path) = write_by_content(dir, file, &text) {
            sdk.replacements.insert(original, path);
        }
    }
}

/// Recover the canonical source only for a byte-exact current compiler overlay.
/// The caller must still verify the recovered canonical source's identity.
pub(crate) fn original_if_current_patch(file: &str, selected: &str) -> Option<String> {
    if std::env::var("DARTFORGE_JSPROD_SDK_PATCHES").is_ok_and(|v| v == "0") {
        return None;
    }
    let edits: Vec<_> = PATCHES
        .iter()
        .filter(|(switch, path, _, _)| {
            *path == file && !switch.is_some_and(|v| std::env::var(v).is_ok_and(|v| v == "0"))
        })
        .map(|(_, _, old, new)| (*old, *new))
        .collect();
    reverse_and_roundtrip(selected, &edits)
}

fn reverse_and_roundtrip(selected: &str, edits: &[(&str, &str)]) -> Option<String> {
    let mut canonical = selected.to_owned();
    for &(old, new) in edits.iter().rev() {
        if canonical.matches(new).count() == 1 {
            canonical = canonical.replacen(new, old, 1);
        }
    }
    (apply_edits(&canonical, edits)?.as_str() == selected).then_some(canonical)
}

fn apply_edits(source: &str, edits: &[(&str, &str)]) -> Option<String> {
    let mut text = source.to_owned();
    let mut changed = false;
    for &(old, new) in edits {
        let crlf = text.contains("\r\n");
        let (old, new) = if crlf {
            (old.replace('\n', "\r\n"), new.replace('\n', "\r\n"))
        } else {
            (old.to_owned(), new.to_owned())
        };
        if text.matches(&old).count() == 1 {
            text = text.replacen(&old, &new, 1);
            changed = true;
        }
    }
    changed.then_some(text)
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
    fn inverse_requires_complete_exact_roundtrip() {
        let edits = [
            ("first-original", "first-patched"),
            ("second-original", "second-patched"),
        ];
        let original = "first-original; second-original";
        let selected = super::apply_edits(original, &edits).unwrap();
        assert_eq!(
            super::reverse_and_roundtrip(&selected, &edits).as_deref(),
            Some(original)
        );
        assert!(super::reverse_and_roundtrip("first-patched; second-original", &edits).is_none());
        assert!(super::reverse_and_roundtrip("first-original; second-patched", &edits).is_none());
        assert!(super::reverse_and_roundtrip(original, &edits).is_none());
        let mutated = format!("{selected}; unexpected-mutation");
        assert_ne!(
            super::reverse_and_roundtrip(&mutated, &edits).as_deref(),
            Some(original)
        );
    }

    #[test]
    fn every_patch_changes_its_text() {
        for (_, file, old, new) in PATCHES {
            assert_ne!(old, new, "{file}");
            assert!(!old.is_empty(), "{file}");
        }
    }
}
