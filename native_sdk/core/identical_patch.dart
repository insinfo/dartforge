// Replaces `_internal/vm/lib/identical_patch.dart` (overlay `native_sdk/`,
// P5d): the original content, plus the `_dartforge*` functions with which the
// DartForge runtime builds the errors it throws itself (a failing cast,
// `assert`, range, `!` on null) as the source's objects — the `dart:core`
// registration hands their addresses to the runtime
// (`dartforge_register_helper`).

part of "core_patch.dart";

@patch
@pragma("vm:recognized", "other")
@pragma("vm:exact-result-type", bool)
@pragma("vm:external-name", "Identical_comparison")
external bool identical(Object? a, Object? b);

@patch
@pragma("vm:entry-point", "call")
int identityHashCode(Object? object) => object._identityHashCode;

@pragma("vm:entry-point")
Object _dartforgeTypeError(String message) =>
    new _TypeError._create("", 0, 0, message);

@pragma("vm:entry-point")
Object _dartforgeAssertionError(Object? message) => new AssertionError(message);

@pragma("vm:entry-point")
Object _dartforgeRangeError(int value, int minimum, int maximum, String? name) =>
    new RangeError.range(value, minimum, maximum, name);

@pragma("vm:entry-point")
Object _dartforgeStateError(String message) => new StateError(message);

// The `FormatException` the runtime throws (`int.parse`, the zlib filters):
// the SDK's real class, which the program's `catch` and `e.message` see.
@pragma("vm:entry-point")
Object _dartforgeFormatError(String message, Object? source, int offset) =>
    new FormatException(message, source, offset < 0 ? null : offset);

@pragma("vm:entry-point")
Object _dartforgeUnsupportedError(String? message) =>
    new UnsupportedError(message ?? "");

@pragma("vm:entry-point")
Object _dartforgeArgumentError(String? message, String? name) =>
    new ArgumentError(message, name);

// Calling a closure with an incompatible arity must throw the SDK's real
// class; the runtime's synthetic id 1012 takes part neither in method
// dispatch nor in the subtype graph of the library compiled from source.
@pragma("vm:entry-point")
Object _dartforgeCallError(String name) =>
    new NoSuchMethodError.withInvocation(
        null, new Invocation.method(new Symbol(name), const []));

/// The selector the receiver's class does not have (`type`: 0 method, 1
/// getter, 2 setter): as on the VM, the receiver's `noSuchMethod` gets the
/// `Invocation`; its result is the call's.
@pragma("vm:entry-point")
Object? _dartforgeNoSuchMethod(
    Object? receiver,
    int type,
    String name,
    List<Object?> positional,
    List<String> names,
    List<Object?> values,
    List<Type> types) {
  final Invocation invocation;
  if (type == 1) {
    invocation = new Invocation.getter(new Symbol(name));
  } else if (type == 2) {
    invocation = new Invocation.setter(
        new Symbol("$name="), positional.isEmpty ? null : positional[0]);
  } else {
    final named = <Symbol, Object?>{};
    for (var i = 0; i < names.length; i++) {
      named[new Symbol(names[i])] = values[i];
    }
    invocation = new Invocation.genericMethod(
        new Symbol(name), types.isEmpty ? null : types, positional, named);
  }
  return receiver.noSuchMethod(invocation);
}

@pragma("vm:entry-point")
Object _dartforgeEmptyStackTrace() => StackTrace.empty;

/// The `Uri.base` hook the embedder installs at startup
/// (`DartUtils::PrepareCoreLibrary`: `dart:io`'s `_getUriBaseClosure`).
@pragma("vm:entry-point")
void _dartforgeSetBaseUri(_UriBaseClosure hook) {
  _uriBaseClosure = hook;
}
