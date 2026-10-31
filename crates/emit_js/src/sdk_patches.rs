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
        "      name = _Utils.substring(name,\n          _Utils.stringIndexOf(name, Recipe.librarySeparatorString, 0) + 1, name.length);",
    ),
    // `_rtiToString` grows and reads the generic context, a `List<String>`
    // it owns, with the checked list operations: `add` kept `checkGrowable`
    // and `UnsupportedError`, `[]` kept `diagnoseIndexError` and
    // `IndexError` in every program. The indices are in range by
    // construction; the JavaScript `push` and index are the same operations.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_shared/lib/rti.dart",
        "      genericContext.add('T${offset + i}');",
        "      JS('', '#.push(#)', genericContext, 'T${offset + i}');",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_shared/lib/rti.dart",
        "      typeParametersText += genericContext[genericContext.length - 1 - i];",
        "      typeParametersText += JS<String>(\n          'String', '#[#]', genericContext, genericContext.length - 1 - i);",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_shared/lib/rti.dart",
        "    return context[context.length - 1 - index];",
        "    return JS<String>('String', '#[#]', context, context.length - 1 - index);",
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
    // Hot restart (`dart.hotRestart`, the development loader's) resets the
    // lazy fields through closures each field records on first use, and the
    // debugger settings (`ddcSettings`) only turn on `trackProfile`: the
    // single production file never restarts nor profiles.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        r##"  // Tracks if these local variables have been saved so they can be restored
  // after a hot restart.
  let savedLocals = false;
  $desc.get = function() {
    if (init == null) return value;
    if (final && initialized) $throwLateInitializationError($name);
    if (!savedLocals) {
      // Record the field on first execution so we can reset it later if
      // needed (hot restart).
      $resetFields.push(() => {
        init = initializer;
        value = null;
        savedLocals = false;
        initialized = false;
      });
      savedLocals = true;
    }
"##,
        r##"  $desc.get = function() {
    if (init == null) return value;
    if (final && initialized) $throwLateInitializationError($name);
"##,
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        r##"  if (setter != null) {
    $desc.set = function(x) {
      if (!savedLocals) {
        $resetFields.push(() => {
          init = initializer;
          value = null;
          savedLocals = false;
          initialized = false;
        });
        savedLocals = true;
      }
      init = null;"##,
        r##"  if (setter != null) {
    $desc.set = function(x) {
      init = null;"##,
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/runtime.dart",
        r##"    // These settings must be configured before the application starts so that
    // user code runs with the correct configuration.
    let settings = 'ddcSettings' in globalState ? globalState.ddcSettings : {};

    $trackProfile(
        'trackProfile' in settings ? settings.trackProfile : false);

"##,
        // Not empty: the fresh-list proof reverses each patch to recognize
        // the canonical source (`original_if_current_patch`).
        r##"    // Production: no ddcSettings (the development loader's).

"##,
    ),
    // `_applyExtension` refuses to install a class onto the JavaScript
    // `Object`, with a warning. `registerExtension` passes `global[name]`
    // for the names of the SDK's `@Native` classes, never `Object`; only
    // `applyExtension` (a native object's constructor) and
    // `applyExtensionForTesting` can pass it, and the check moves there.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/classes.dart",
        "  if (JS('!', '# === #.Object', jsType, global_)) {\n    var extName = JS<String>('!', '#.name', dartExtType);\n    _warn(\n        \"Attempting to install properties from non-Object type '$extName' onto the native JS Object.\");\n    return;\n  }\n",
        "  // Production: `_ontoObject` checks the JavaScript `Object` in the callers\n  // that can pass it.\n",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/classes.dart",
        "  var jsType = JS('', '#.constructor', nativeObject);\n  _applyExtension(jsType, dartExtType);\n",
        "  var jsType = JS('', '#.constructor', nativeObject);\n  if (_ontoObject(jsType, dartExtType)) return;\n  _applyExtension(jsType, dartExtType);\n}\n\n/// `_applyExtension`'s refusal to install a class onto the JavaScript\n/// `Object` (production: only these callers can pass it).\nbool _ontoObject(jsType, dartExtType) {\n  if (jsType == null) return false;\n  if (JS<Object?>('', '#.prototype', jsType) == null) return false;\n  if (JS('!', '# === #', dartExtType, JS_CLASS_REF(Object))) return false;\n  if (JS('!', '# === #.Object', jsType, global_)) {\n    var extName = JS<String>('!', '#.name', dartExtType);\n    _warn(\n        \"Attempting to install properties from non-Object type '$extName' onto the native JS Object.\");\n    return true;\n  }\n  return false;\n",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/classes.dart",
        "applyExtensionForTesting(name) {\n  var dartExtType = JS('', '#.get(#)', _extensionMap, name);\n  var jsType = JS('', '#[#]', global_, name);\n  _applyExtension(jsType, dartExtType);\n",
        "applyExtensionForTesting(name) {\n  var dartExtType = JS('', '#.get(#)', _extensionMap, name);\n  var jsType = JS('', '#[#]', global_, name);\n  if (_ontoObject(jsType, dartExtType)) return;\n  _applyExtension(jsType, dartExtType);\n",
    ),
    // `_installProperties` copies `Object`'s members onto the JavaScript
    // prototype and then each subclass's, redefining the overridden ones
    // (`toString`, the `runtimeType` getter) with other attributes. V8 turns
    // a prototype whose property is reconfigured into a dictionary, and
    // nothing turns the prototypes of `Number` and `String` back (only an
    // inline cache on an object receiver does that, and their receivers are
    // primitives): every extension method call on a number or a string
    // became a slow lookup. The members are collected on a fresh object and
    // defined once each.
    (
        Some("DARTFORGE_JSPROD_FAST_PROTOTYPES"),
        "_internal/js_dev_runtime/private/ddc_runtime/classes.dart",
        "  _installProperties(\n      jsProto, dartExtType, JS('', '#[#]', jsProto, _extensionType));\n",
        "  // Production: collected first, so that each member is defined once on\n  // `jsProto` (a redefinition makes V8 turn the prototype into a dictionary).\n  var staged = JS<Object>('!', 'Object.create(null)');\n  _installProperties(\n      staged, dartExtType, JS('', '#[#]', jsProto, _extensionType));\n  copyTheseProperties(jsProto, staged, getOwnPropertySymbols(staged));\n",
    ),
    // `dart.global` falls back to `new Function('return this;')()` when no
    // `window`, `global` or `self` exists; the production file defines
    // `self` before anything else runs (`bundle.rs`), so the fallback is
    // never taken.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/runtime.dart",
        "      : (typeof self != \"undefined\") ? self : null;\n    if (!globalState) {\n      // Some platforms (e.g., d8) do not define any of the above.  The\n      // following is a non-CSP safe way to access the global object:\n      globalState = new Function('return this;')();\n    }\n",
        "      : self; // Production: the file defines `self` first.\n",
    ),
    // `Error.safeToString` tests the primitives and strings and then calls
    // `_objectToString`, `Primitives.safeToString`, which runs the same
    // tests with the same results (`toString` of a number, `bool` or
    // `null`, `JSON.stringify` of a string) before describing an object:
    // the first copy of the tests goes.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "core/errors.dart",
        "  static String safeToString(Object? object) {\n    if (object is num || object is bool || null == object) {\n      return object.toString();\n    }\n    if (object is String) {\n      return _stringToSafeString(object);\n    }\n    return _objectToString(object);\n  }\n",
        "  static String safeToString(Object? object) {\n    // Production: `_objectToString` runs the same tests.\n    return _objectToString(object);\n  }\n",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "core/errors.dart",
        "  external static String _objectToString(Object object);\n",
        "  external static String _objectToString(Object? object);\n",
    ),
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/patch/core_patch.dart",
        "  static String _objectToString(Object object) {\n    return Primitives.safeToString(object);\n  }\n",
        "  static String _objectToString(Object? object) {\n    return Primitives.safeToString(object);\n  }\n",
    ),
    // `getInterceptorForRti` gives every branch a class (`Null`, the two
    // number classes, `Function` or `JavaScriptFunction`, the prototype's
    // extension type or `LegacyJavaScriptObject`): its final throw for a
    // missing one is never reached.
    (
        Some("DARTFORGE_JSPROD_FLOOR_PATCHES"),
        "_internal/js_dev_runtime/private/ddc_runtime/rtti.dart",
        "  if (classRef == null) throw 'Unknown interceptor for object: ($obj)';\n  return JS<Object>('!', '#.prototype', classRef);\n",
        "  // Production: every branch above gives a class.\n  return JS<Object>('!', '#.prototype', classRef);\n",
    ),
    // With `Object`'s `dartx.toString` forwarding to the JavaScript
    // `toString` (`module.rs::object_member_forwarders`), a tear-off through
    // the symbol would bind the forwarder; for a Dart object it binds the
    // JavaScript `toString` instead, the method every other tear-off of
    // `toString` binds, so that the tear-offs stay equal.
    (
        Some("DARTFORGE_JSPROD_TOSTRING_FORWARDER"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "  if (obj == null ||\n      JS<bool>('!', '#[#] !== void 0', obj, extensionSymbol('toString'))) {\n    // The bind helper can handle finding the toString method for null or Dart\n    // Objects.\n    return bind(obj, extensionSymbol('toString'), null);\n  }\n",
        "  if (obj == null ||\n      JS<bool>('!', '#[#] !== void 0 && !(# instanceof #)', obj,\n          extensionSymbol('toString'), obj, JS_CLASS_REF(Object))) {\n    // Production: a Dart object binds its JavaScript `toString`.\n    return bind(obj, extensionSymbol('toString'), null);\n  }\n",
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
    ), // `Object.hash` dispatches on how many arguments it got (a sentinel
    // default for each omitted one), which keeps every arity and every
    // `SystemHash.hashN` alive. One function per arity, called directly by
    // the production profile at each call site
    // (`Ctx::arity_specialization`); the original stays for tear-offs.
    // `strSafe` (the interpolation of a number) calls the number's Dart
    // `toString` through its extension symbol and throws an `ArgumentError`
    // when the result is not a `String`, which a number's never is not: the
    // error path, the installed `toString` and with them the universal
    // `toString` stayed in every program that interpolates an `int`.
    // `strNum` is `JSNumber.toString` itself, called directly by the
    // production profile for a non-nullable `int`, `double` or `num`.
    (
        Some("DARTFORGE_JSPROD_STR_PRIMITIVES"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "/// Helper method for `.noSuchMethod` used when the receiver isn't statically\n",
        "/// `JSNumber.toString` of a number (the production profile's interpolation\n/// of a non-nullable `int`, `double` or `num`).\n@notNull\nString strNum(@notNull num n) =>\n    JS<String>('!', '# === 0 && 1 / # < 0 ? \"-0.0\" : \"\" + #', n, n, n);\n\n/// Helper method for `.noSuchMethod` used when the receiver isn't statically\n",
    ),
    // `>>` by a literal count (`emit_js`'s literal shifts) needs the receiver
    // twice: the body of `JSNumber.>>` for a count up to 31 as a function.
    (
        Some("DARTFORGE_JSPROD_LITERAL_SHIFTS"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "/// Helper method for `.noSuchMethod` used when the receiver isn't statically\n",
        "/// `JSNumber.>>` of `n` by a `count` from 0 to 31 (the production\n/// profile's shifts by a literal count).\n@notNull\nint shrCount(@notNull int n, @notNull int count) =>\n    JS<int>('!', '# > 0 ? # >>> # : (# >> #) >>> 0', n, n, count, n, count);\n\n/// Helper method for `.noSuchMethod` used when the receiver isn't statically\n",
    ),
    // `print` converts its argument with the interpolation's generic
    // `dart.str`, whose `toString` reaches every class. The conversion moves
    // to the call site (`emit_js`'s print specialization calls `_printLine`
    // with the argument converted as an interpolation would, by its static
    // type); `print` itself stays for tear-offs.
    (
        Some("DARTFORGE_JSPROD_PRINT_LINE"),
        "core/print.dart",
        "void print(Object? object) {\n  String line = \"$object\";\n  var toZone = printToZone;",
        "void print(Object? object) {\n  _printLine(\"$object\");\n}\n\n/// `print` of a line already converted (the production profile's call\n/// sites of `print`).\nvoid _printLine(String line) {\n  var toZone = printToZone;",
    ),
    // Positional class type variables (`Ctx::positional_type_variables`):
    // the production profile names a class type variable by its position
    // (`C.0`) and leaves the substitution entries out of the rti rules.
    // `evalTypeVariable` derives them: in an environment of class `C` the
    // variable `k` is the type argument `k + 1`, and in a subclass it is the
    // `k`-th argument of the subclass's `C` supertype entry, which is what
    // the left-out entry held (`module.rs::emit_rules_with` builds both
    // from the same arguments).
    (
        Some("DARTFORGE_JSPROD_POSITIONAL_TYPE_VARIABLES"),
        "_internal/js_shared/lib/rti.dart",
        "    String interfaceName = Rti._getInterfaceName(environment);\n    var rule = _Universe.findRule(universe, interfaceName);\n    assert(rule != null);\n    String? recipe = TypeRule.lookupTypeVariable(rule, name);\n",
        "    String interfaceName = Rti._getInterfaceName(environment);\n    String? recipe = _positionalTypeVariable(universe, interfaceName, name);\n",
    ),
    (
        Some("DARTFORGE_JSPROD_POSITIONAL_TYPE_VARIABLES"),
        "_internal/js_shared/lib/rti.dart",
        "/// Represents the set of supertypes and type variable bindings for a given\n/// target type. The target type itself is not stored on the [TypeRule].\nclass TypeRule {\n",
        "/// The recipe of the class type variable `name`, written `C.k` with `k`\n/// its position among the type parameters of `C` (the production profile's\n/// recipes), in an environment of class `interfaceName`.\n/// A class recipe has no period (`|h`, `lib|C`), so the name splits in two.\nString? _positionalTypeVariable(\n    Object? universe, String interfaceName, String name) {\n  var parts = JS('', '#.split(\".\")', name);\n  return JS('', '#[0] === # ? \"\" + (+#[1] + 1) : #[#[0]]?.[#[1]]', parts,\n      interfaceName, parts, _Universe.findRule(universe, interfaceName), parts,\n      parts);\n}\n\n/// Represents the set of supertypes and type variable bindings for a given\n/// target type. The target type itself is not stored on the [TypeRule].\nclass TypeRule {\n",
    ),
    // `_isSubtype` tests the kind of `t` against the function kinds through
    // two constants of their own, equal to `kindFunction` and
    // `kindGenericFunction`: when no function type is ever the target of a
    // subtype test (closure types are only ever the `s` side), the
    // production world folds exactly those two tests, and with them
    // `_isFunctionSubtype` (`dartforge_world::Constants::set_without_function_targets`).
    //
    // The parser reads a function type followed by type arguments as a
    // generic function type through a constant of its own, equal to
    // `kindFunction`: when no generic function type exists, the world folds
    // that `case` and with it the substitution of generic function types
    // (`dartforge_world::Constants::set_without_generic_functions`).
    (
        None,
        "_internal/js_shared/lib/rti.dart",
        "  static const int kindGenericFunctionParameter = 14;\n",
        "  static const int kindGenericFunctionParameter = 14;\n  static const int kindFunctionTarget = kindFunction;\n  static const int kindGenericFunctionTarget = kindGenericFunction;\n  static const int kindGenericFunctionHead = kindFunction;\n",
    ),
    (
        Some("DARTFORGE_JSPROD_GENERIC_FUNCTIONS_GATE"),
        "_internal/js_shared/lib/rti.dart",
        "      switch (Rti._getKind(base)) {\n        case Rti.kindFunction:\n          push(\n              stack,\n              _Universe._lookupGenericFunctionRti(",
        "      switch (Rti._getKind(base)) {\n        case Rti.kindGenericFunctionHead:\n          push(\n              stack,\n              _Universe._lookupGenericFunctionRti(",
    ),
    (
        Some("DARTFORGE_JSPROD_FUNCTION_TARGETS_GATE"),
        "_internal/js_shared/lib/rti.dart",
        "  if (tKind == Rti.kindGenericFunction) {\n    if (isJsFunctionType(s)) return true;\n",
        "  if (tKind == Rti.kindGenericFunctionTarget) {\n    if (isJsFunctionType(s)) return true;\n",
    ),
    (
        Some("DARTFORGE_JSPROD_FUNCTION_TARGETS_GATE"),
        "_internal/js_shared/lib/rti.dart",
        "  if (tKind == Rti.kindFunction) {\n    if (isJsFunctionType(s)) return true;\n",
        "  if (tKind == Rti.kindFunctionTarget) {\n    if (isJsFunctionType(s)) return true;\n",
    ),
    (
        Some("DARTFORGE_JSPROD_HASH_ARITY"),
        "core/object.dart",
        "final int _hashSeed = identityHashCode(Object);\n",
        r##"final int _hashSeed = identityHashCode(Object);

// `Object.hash` by number of arguments: the production profile calls these
// directly (`emit_js`'s arity specialization), so the other arities and
// their `SystemHash` combinations are not reached.
int _hash2(Object? o1, Object? o2) =>
    SystemHash.hash2(o1.hashCode, o2.hashCode, _hashSeed);
int _hash3(Object? o1, Object? o2, Object? o3) =>
    SystemHash.hash3(o1.hashCode, o2.hashCode, o3.hashCode, _hashSeed);
int _hash4(Object? o1, Object? o2, Object? o3, Object? o4) =>
    SystemHash.hash4(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, _hashSeed);
int _hash5(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5) =>
    SystemHash.hash5(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, _hashSeed);
int _hash6(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6) =>
    SystemHash.hash6(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, _hashSeed);
int _hash7(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7) =>
    SystemHash.hash7(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, _hashSeed);
int _hash8(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8) =>
    SystemHash.hash8(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, _hashSeed);
int _hash9(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9) =>
    SystemHash.hash9(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, _hashSeed);
int _hash10(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10) =>
    SystemHash.hash10(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, _hashSeed);
int _hash11(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11) =>
    SystemHash.hash11(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, _hashSeed);
int _hash12(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12) =>
    SystemHash.hash12(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, _hashSeed);
int _hash13(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12, Object? o13) =>
    SystemHash.hash13(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, o13.hashCode, _hashSeed);
int _hash14(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12, Object? o13, Object? o14) =>
    SystemHash.hash14(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, o13.hashCode, o14.hashCode, _hashSeed);
int _hash15(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12, Object? o13, Object? o14, Object? o15) =>
    SystemHash.hash15(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, o13.hashCode, o14.hashCode, o15.hashCode, _hashSeed);
int _hash16(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12, Object? o13, Object? o14, Object? o15, Object? o16) =>
    SystemHash.hash16(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, o13.hashCode, o14.hashCode, o15.hashCode, o16.hashCode, _hashSeed);
int _hash17(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12, Object? o13, Object? o14, Object? o15, Object? o16, Object? o17) =>
    SystemHash.hash17(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, o13.hashCode, o14.hashCode, o15.hashCode, o16.hashCode, o17.hashCode, _hashSeed);
int _hash18(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12, Object? o13, Object? o14, Object? o15, Object? o16, Object? o17, Object? o18) =>
    SystemHash.hash18(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, o13.hashCode, o14.hashCode, o15.hashCode, o16.hashCode, o17.hashCode, o18.hashCode, _hashSeed);
int _hash19(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12, Object? o13, Object? o14, Object? o15, Object? o16, Object? o17, Object? o18, Object? o19) =>
    SystemHash.hash19(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, o13.hashCode, o14.hashCode, o15.hashCode, o16.hashCode, o17.hashCode, o18.hashCode, o19.hashCode, _hashSeed);
int _hash20(Object? o1, Object? o2, Object? o3, Object? o4, Object? o5, Object? o6, Object? o7, Object? o8, Object? o9, Object? o10, Object? o11, Object? o12, Object? o13, Object? o14, Object? o15, Object? o16, Object? o17, Object? o18, Object? o19, Object? o20) =>
    SystemHash.hash20(o1.hashCode, o2.hashCode, o3.hashCode, o4.hashCode, o5.hashCode, o6.hashCode, o7.hashCode, o8.hashCode, o9.hashCode, o10.hashCode, o11.hashCode, o12.hashCode, o13.hashCode, o14.hashCode, o15.hashCode, o16.hashCode, o17.hashCode, o18.hashCode, o19.hashCode, o20.hashCode, _hashSeed);
"##,
    ),
    // `-O3`/`-O4` (`dart2js`'s `--omit-implicit-checks`): a dynamic call
    // checks the arity and the names of its arguments (`NoSuchMethodError`)
    // but not their types, nor the type arguments against the bounds, and a
    // dynamic `x.f = v` does not check `v` against the setter's type: the
    // parameter checks of `dart2js` that the level trusts.
    // `JS_GET_FLAG('OMIT_IMPLICIT_CHECKS')` is the level
    // (`emit_js::own_sdk`, `dartforge_world::Constants`).
    (
        None,
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "  // Now that we know the signature matches, we can perform type checks.\n  for (var i = 0; i < requiredCount; ++i) {",
        "  // Now that we know the signature matches, we can perform type checks.\n  if (JS_GET_FLAG('OMIT_IMPLICIT_CHECKS')) return null;\n  for (var i = 0; i < requiredCount; ++i) {",
    ),
    (
        None,
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "      // Check the provided type arguments against the instantiated bounds.\n      for (var i = 0; i < typeParameterCount; i++) {",
        "      // Check the provided type arguments against the instantiated bounds.\n      if (!JS_GET_FLAG('OMIT_IMPLICIT_CHECKS')) for (var i = 0; i < typeParameterCount; i++) {",
    ),
    (
        None,
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "    if (setterType != null) {\n      return JS('', '#[#] = #.#(#)', obj, f, setterType,",
        "    if (setterType != null) {\n      if (JS_GET_FLAG('OMIT_IMPLICIT_CHECKS')) {\n        return JS('', '#[#] = #', obj, f, value);\n      }\n      return JS('', '#[#] = #.#(#)', obj, f, setterType,",
    ),
    // The type parameters of a generic function type print as on the VM
    // (`TypeParameter::CanonicalNameCString`): `Y0`, `Y1`… and, inside
    // `n` enclosing type parameters, `F<n>Y0`…, where `dart2js` and DDC
    // print `T<n + 1>`.
    (
        None,
        "_internal/js_shared/lib/rti.dart",
        "      genericContext.add('T${offset + i}');\n",
        "      genericContext.add(offset == 0 ? 'Y${i - 1}' : 'F${offset}Y${i - 1}');\n",
    ),
    // The same line after the `push` patch above.
    (
        None,
        "_internal/js_shared/lib/rti.dart",
        "      JS('', '#.push(#)', genericContext, 'T${offset + i}');\n",
        "      JS('', '#.push(#)', genericContext,\n          offset == 0 ? 'Y${i - 1}' : 'F${offset}Y${i - 1}');\n",
    ),
    // A dynamic call names a program library's private member by its
    // production property (`_other$1c`, the library tag after the name,
    // `crate::production_private_name`): the `Symbol` of the
    // `NoSuchMethodError` and of `Invocation.memberName` shows the Dart
    // name, as on the VM.
    (
        None,
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "      if ($name[0] === '_') {\n        // Inverse of\n",
        "      if ($name[0] === '_') {\n        // A program library's private name in production: `_name` and\n        // the library tag (`\\$` and `[0-9a-z]`). Indexing only: a method\n        // called in a template is a live selector for the closed world.\n        let end = $name.length;\n        while (end > 2 && ($name[end - 1] >= '0' && $name[end - 1] <= '9' ||\n            $name[end - 1] >= 'a' && $name[end - 1] <= 'z')) end--;\n        if (end > 2 && end < $name.length && $name[end - 1] === '\\$') {\n          return $name.substring(0, end - 1);\n        }\n        // Inverse of\n",
    ),
    // The production profile shortens the keys of named parameters
    // (`emit_js_production`'s `named_args`), and a function type's recipe
    // carries the key (sorted by key): when a function type the program may
    // print cites a shortened key, `JS_GET_FLAG('NAMED_SOURCE_NAMES')` is
    // true and the printer shows the source names, in their order, through
    // the bootstrap's table of those keys (`dart.namedSourceName`).
    (
        None,
        "_internal/js_shared/lib/rti.dart",
        "              _Utils.asRti(_Utils.arrayAt(named, i + 2)), genericContext) +\n          ' ' +\n          _Utils.asString(_Utils.arrayAt(named, i));\n",
        "              _Utils.asRti(_Utils.arrayAt(named, i + 2)), genericContext) +\n          ' ' +\n          (JS_GET_FLAG('NAMED_SOURCE_NAMES')\n              ? _namedParameterName(_Utils.asString(_Utils.arrayAt(named, i)))\n              : _Utils.asString(_Utils.arrayAt(named, i)));\n",
    ),
    (
        None,
        "_internal/js_shared/lib/rti.dart",
        "    for (int i = 0; i < namedLength; i += 3) {\n      argumentsText += sep;\n",
        "    for (int k = 0; k < namedLength; k += 3) {\n      int i = JS_GET_FLAG('NAMED_SOURCE_NAMES') ? _namedParameterAt(named, k) : k;\n      argumentsText += sep;\n",
    ),
    (
        None,
        "_internal/js_shared/lib/rti.dart",
        "String _functionRtiToString(Rti functionType, List<String>? genericContext,\n",
        "/// The source name of a named parameter whose key the production profile\n/// shortened (`dart.namedSourceName`).\nString _namedParameterName(String key) =>\n    JS('String', 'dart.namedSourceName(#)', key);\n\n/// The position (`0, 3, 6…`) in `named` (triples of key, `required` and\n/// type, sorted by key) of the `k ~/ 3`-th named parameter in the order of\n/// the source names, the order the VM prints (an insertion sort: a method\n/// called in a template is a live selector for the closed world).\nint _namedParameterAt(Object named, int k) => JS('int', '''(() => {\n  let order = [], n = 0;\n  for (let i = 0; i < $named.length; i += 3) {\n    let x = dart.namedSourceName($named[i]), j = n++;\n    while (j > 0 && dart.namedSourceName($named[order[j - 1]]) > x) {\n      order[j] = order[j - 1];\n      j--;\n    }\n    order[j] = i;\n  }\n  return order[$k / 3];\n})()''');\n\nString _functionRtiToString(Rti functionType, List<String>? genericContext,\n",
    ),
    // `IdentityMap` (every map with `String`, `int` or other primitive keys,
    // and the `identity` maps) keeps its entries in cells linked in
    // insertion order and finds them through an object per kind of key, as
    // dart2js does, instead of a JavaScript `Map`: an integer key is an
    // element lookup (`docs/JS-PRODUCAO-VELOCIDADE.md` §10). The other keys
    // stay in a `Map`. Iteration checks the modification count before each
    // step, as before; `-0.0` keeps the key first inserted, a `NaN` key is
    // never found again and `addAll` counts only new keys, as on the VM.
    (
        Some("DARTFORGE_JSPROD_IDENTITY_MAP_CELLS"),
        "_internal/js_dev_runtime/private/identity_hash_map.dart",
        r##"base class IdentityMap<K, V> extends InternalMap<K, V> {
  final _map = JS('', 'new Map()');

  // We track the number of modifications done to the key set of the
  // hash map to be able to throw when the map is modified while being
  // iterated over.
  //
  // Value cycles after 2^30 modifications so that modification counts are
  // always unboxed (Smi) values. Modification detection will be missed if you
  // make exactly some multiple of 2^30 modifications between advances of an
  // iterator.
  @notNull
  int _modifications = 0;

  IdentityMap();
  IdentityMap.from(JSArray entries) {
    var map = _map;
    for (int i = 0, n = JS<int>('!', '#.length', entries); i < n; i += 2) {
      JS('', '#.set(#[#], #[#])', map, entries, i, entries, i + 1);
    }
  }

  int get length => JS<int>('!', '#.size', _map);
  bool get isEmpty => JS<bool>('!', '#.size == 0', _map);
  bool get isNotEmpty => JS<bool>('!', '#.size != 0', _map);

  Iterable<K> get keys => _JSMapIterable<K>(this, true);
  Iterable<V> get values => _JSMapIterable<V>(this, false);

  bool containsKey(Object? key) {
    return JS<bool>('!', '#.has(#)', _map, key);
  }

  bool containsValue(Object? value) {
    for (var v in JS('', '#.values()', _map)) {
      if (v == value) return true;
    }
    return false;
  }

  void addAll(Map<K, V> other) {
    if (other.isNotEmpty) {
      var map = _map;
      other.forEach((key, value) {
        JS('', '#.set(#, #)', map, key, value);
      });
      _modifications = (_modifications + 1) & 0x3fffffff;
    }
  }

  V? operator [](Object? key) {
    V value = JS('', '#.get(#)', _map, key);
    // coerce undefined to null.
    return JS<bool>('!', '# === void 0', value) ? null : value;
  }

  void operator []=(K key, V value) {
    var map = _map;
    int length = JS('!', '#.size', map);
    JS('', '#.set(#, #)', map, key, value);
    if (length != JS<int>('!', '#.size', map)) {
      _modifications = (_modifications + 1) & 0x3fffffff;
    }
  }

  V putIfAbsent(K key, V ifAbsent()) {
    if (JS<bool>('!', '#.has(#)', _map, key)) {
      return JS('', '#.get(#)', _map, key);
    }
    V value = ifAbsent();
    if (value == null) JS('', '# = null', value);
    JS('', '#.set(#, #)', _map, key, value);
    _modifications = (_modifications + 1) & 0x3fffffff;
    return value;
  }

  V? remove(Object? key) {
    V value = JS('', '#.get(#)', _map, key);
    if (JS<bool>('!', '#.delete(#)', _map, key)) {
      _modifications = (_modifications + 1) & 0x3fffffff;
    }
    // coerce undefined to null.
    return JS<bool>('!', '# === void 0', value) ? null : value;
  }

  void clear() {
    if (JS<int>('!', '#.size', _map) > 0) {
      JS('', '#.clear()', _map);
      _modifications = (_modifications + 1) & 0x3fffffff;
    }
  }
}

"##,
        r##"base class IdentityMap<K, V> extends InternalMap<K, V> {
  // The entries are cells `{k, v, n, p}` (key, value, next, previous) in a
  // list in insertion order, found by key through three tables, as in the
  // `JsLinkedHashMap` of dart2js: `String` keys in `_strings` and the
  // integers `k` with `(k & 0x3fffffff) === k` in `_nums`, objects without
  // prototype; every other key in the JavaScript `Map` `_rest`. As on the
  // VM, `-0.0` and `0` are one key (the cell keeps the first one) and a
  // `NaN` key is never found again: its cell is in no table.
  Object? _strings;
  Object? _nums;
  Object? _rest;
  Object? _first;
  Object? _last;
  @notNull
  int _length = 0;

  // Value cycles after 2^30 modifications so that modification counts are
  // always unboxed (Smi) values. A field of its own: `_modifications`
  // overrides an abstract getter, which makes it an accessor pair.
  @notNull
  int _mods = 0;

  IdentityMap();
  IdentityMap.from(JSArray entries) {
    for (int i = 0, n = JS<int>('!', '#.length', entries); i < n; i += 2) {
      _put(JS('', '#[#]', entries, i), JS('', '#[#]', entries, i + 1));
    }
  }

  int get _modifications => _mods;

  /// What `_JSMapIterable` reads: `keys()` and `values()` return JavaScript
  /// iterators over the cells from the current first one.
  get _map => JS('', '''((c) => {
    let it = (keys) => ({next() {
      if (c == null) return {value: void 0, done: true};
      let x = c;
      c = c.n;
      return {value: keys ? x.k : x.v, done: false};
    }});
    return {keys: () => it(true), values: () => it(false)};
  })(#)''', _first);

  int get length => _length;
  bool get isEmpty => _length == 0;
  bool get isNotEmpty => _length != 0;

  Iterable<K> get keys => _JSMapIterable<K>(this, true);
  Iterable<V> get values => _JSMapIterable<V>(this, false);

  /// The cell of [key], or `null`/`undefined`.
  Object? _cell(Object? key) {
    if (JS<bool>('!', 'typeof # == "string"', key)) {
      var t = _strings;
      return t == null ? null : JS('', '#[#]', t, key);
    }
    if (JS<bool>('!', 'typeof # == "number" && (# & 0x3fffffff) === #', key,
        key, key)) {
      var t = _nums;
      return t == null ? null : JS('', '#[#]', t, key);
    }
    var t = _rest;
    return t == null ? null : JS('', '#.get(#)', t, key);
  }

  void _put(Object? key, Object? value) {
    if (value == null) JS('', '# = null', value);
    if (JS<bool>('!', 'typeof # == "string"', key)) {
      _putIn(_strings ??= JS('', 'Object.create(null)'), key, value);
    } else if (JS<bool>(
        '!', 'typeof # == "number" && (# & 0x3fffffff) === #', key, key, key)) {
      _putIn(_nums ??= JS('', 'Object.create(null)'), key, value);
    } else {
      var t = _rest ??= JS('', 'new Map()');
      var c = JS('', '#.get(#)', t, key);
      if (c != null) {
        JS('', '#.v = #', c, value);
      } else {
        c = _link(key, value);
        // A `NaN` key stays out of the table.
        if (JS<bool>('!', '# === #', key, key)) JS('', '#.set(#, #)', t, key, c);
      }
    }
  }

  void _putIn(Object table, Object? key, Object? value) {
    var c = JS('', '#[#]', table, key);
    if (c != null) {
      JS('', '#.v = #', c, value);
    } else {
      JS('', '#[#] = #', table, key, _link(key, value));
    }
  }

  /// A new cell at the end of the list.
  Object _link(Object? key, Object? value) {
    var last = _last;
    var c = JS<Object>('!', '{k: #, v: #, n: null, p: #}', key, value, last);
    if (last == null) {
      _first = c;
    } else {
      JS('', '#.n = #', last, c);
    }
    _last = c;
    _length++;
    _mods = (_mods + 1) & 0x3fffffff;
    return c;
  }

  bool containsKey(Object? key) => _cell(key) != null;

  bool containsValue(Object? value) {
    for (var c = _first; c != null; c = JS('', '#.n', c)) {
      if (JS('', '#.v', c) == value) return true;
    }
    return false;
  }

  void addAll(Map<K, V> other) {
    other.forEach((K key, V value) {
      _put(key, value);
    });
  }

  V? operator [](Object? key) {
    var c = _cell(key);
    return c == null ? null : JS('', '#.v', c);
  }

  void operator []=(K key, V value) {
    _put(key, value);
  }

  V putIfAbsent(K key, V ifAbsent()) {
    var c = _cell(key);
    if (c != null) return JS('', '#.v', c);
    V value = ifAbsent();
    if (value == null) JS('', '# = null', value);
    // `ifAbsent` may have added the key: `[]=`, as on the VM.
    _put(key, value);
    return value;
  }

  V? remove(Object? key) {
    var c = _cell(key);
    if (c == null) return null;
    if (JS<bool>('!', 'typeof # == "string"', key)) {
      JS('', 'delete #[#]', _strings, key);
    } else if (JS<bool>(
        '!', 'typeof # == "number" && (# & 0x3fffffff) === #', key, key, key)) {
      JS('', 'delete #[#]', _nums, key);
    } else {
      JS('', '#.delete(#)', _rest, key);
    }
    var p = JS('', '#.p', c);
    var n = JS('', '#.n', c);
    if (p == null) {
      _first = n;
    } else {
      JS('', '#.n = #', p, n);
    }
    if (n == null) {
      _last = p;
    } else {
      JS('', '#.p = #', n, p);
    }
    _length--;
    _mods = (_mods + 1) & 0x3fffffff;
    return JS('', '#.v', c);
  }

  void clear() {
    if (_length > 0) {
      _strings = _nums = _rest = _first = _last = null;
      _length = 0;
      _mods = (_mods + 1) & 0x3fffffff;
    }
  }

  void forEach(void action(K key, V value)) {
    int modifications = _mods;
    for (var c = _first; c != null; c = JS('', '#.n', c)) {
      action(JS('', '#.k', c), JS('', '#.v', c));
      if (modifications != _mods) {
        throw ConcurrentModificationError(this);
      }
    }
  }
}

"##,
    ),
    // The `Future` paths behind every `await` (`docs/JS-PRODUCAO-VELOCIDADE.md`
    // §11). A listener of a root-zone future, run in the root zone, calls
    // its callback as `_RootZone.runUnary` does, without evaluating the two
    // type arguments `runUnary` ignores there.
    (
        Some("DARTFORGE_JSPROD_FUTURE_PATHS"),
        "async/future_impl.dart",
        "  FutureOr<T> handleValue(S sourceResult) {\n    return _zone.runUnary<FutureOr<T>, S>(_onValue, sourceResult);\n  }\n",
        "  FutureOr<T> handleValue(S sourceResult) {\n    var zone = _zone;\n    if (identical(zone, _rootZone) && identical(Zone._current, _rootZone)) {\n      return _onValue(sourceResult);\n    }\n    return zone.runUnary<FutureOr<T>, S>(_onValue, sourceResult);\n  }\n",
    ),
    // Completing with a value that is not a `Future` at all (the common
    // case) is decided by the plain `is Future`, without evaluating
    // `Future<T>` in the future's environment: a value that is not a
    // `Future` is not a `Future<T>`.
    (
        Some("DARTFORGE_JSPROD_FUTURE_PATHS"),
        "async/future_impl.dart",
        "    if (value is Future<T>) {\n      _chainFuture(value);\n      return;\n    }\n    _asyncCompleteWithValue(value);\n",
        "    if (value is Future && value is Future<T>) {\n      _chainFuture(value);\n      return;\n    }\n    _asyncCompleteWithValue(unsafeCast<T>(value));\n",
    ),
    (
        Some("DARTFORGE_JSPROD_FUTURE_PATHS"),
        "_internal/js_dev_runtime/patch/async_patch.dart",
        "    } else if (value is Future<T>) {\n      assert(!_future._isComplete);\n      _future._chainFuture(value);\n    } else {\n      _future._completeWithValue(value);\n    }\n",
        "    } else if (value is Future && value is Future<T>) {\n      assert(!_future._isComplete);\n      _future._chainFuture(value);\n    } else {\n      _future._completeWithValue(unsafeCast<T>(value));\n    }\n",
    ),
    // A lazy static becomes a data property once initialized (or first
    // written), as the comment of `defineLazyField` intends: reading
    // `Zone._current` or `_rootZone` is a property load instead of a call
    // to the lazy getter. Writable even when final: Dart code never writes
    // a final static. Only on library objects: V8 turns a class (a
    // function) whose accessor is redefined into a dictionary, and every
    // static of that class gets slower (`json_encode`'s `Stopwatch`).
    (
        Some("DARTFORGE_JSPROD_LAZY_DATA_PROPERTIES"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "    init = null;\n    return value;\n  };\n",
        "    init = null;\n    if (typeof $to != 'function') $defineProperty($to, $name, {value, writable: true});\n    return value;\n  };\n",
    ),
    (
        Some("DARTFORGE_JSPROD_LAZY_DATA_PROPERTIES"),
        "_internal/js_dev_runtime/private/ddc_runtime/operations.dart",
        "      init = null;\n      value = x;\n      setter(x);\n",
        "      init = null;\n      value = x;\n      setter(x);\n      if (typeof $to != 'function') $defineProperty($to, $name, {value, writable: true});\n",
    ),
    // `List.sort()` without a comparator ran the Dart `Sort.sort` with
    // `Comparable.compare(a as Comparable, b as Comparable)` at each
    // comparison. When every element is a JavaScript number (or every one a
    // string), the native `Array.prototype.sort` with the comparator of
    // `num.compareTo` (`-0.0` before `0.0`, `NaN` last) or of
    // `String.compareTo` (code units) sorts the same: two elements compare
    // as 0 only when they are the same value, so the sorted list is the only
    // one, stable sort or not, and the comparator has no effect to order.
    // Anything else (a `null`, a `bool`, objects, a list of one element)
    // goes to `Sort.sort` as before, with its errors.
    (
        Some("DARTFORGE_JSPROD_PRIMITIVE_SORT"),
        "_internal/js_dev_runtime/private/js_array.dart",
        "    if (compare == null) {\n      Sort.sort(\n          this, (a, b) => Comparable.compare(a as Comparable, b as Comparable));\n",
        r#"    if (compare == null) {
      if (JS<bool>('!', r'''((a) => {
        const n = a.length, kind = n > 1 ? typeof a[0] : "";
        if (kind !== "number" && kind !== "string") return false;
        for (let i = 1; i < n; i++) if (typeof a[i] !== kind) return false;
        a.sort(kind === "string" ? (x, y) => x < y ? -1 : x > y ? 1 : 0 :
          (x, y) => x < y ? -1 : x > y ? 1 : x === y ? (x !== 0 || (1 / x < 0) === (1 / y < 0) ? 0 : 1 / x < 0 ? -1 : 1) : x !== x ? (y !== y ? 0 : 1) : -1);
        return true;
      })(#)''', this)) {
        return;
      }
      Sort.sort(
          this, (a, b) => Comparable.compare(a as Comparable, b as Comparable));
"#,
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
