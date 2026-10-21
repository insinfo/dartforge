// The `dart:js_util` patch of DartForge's WebAssembly target
// (`dartforge_wasm`): dart2wasm's (`_internal/wasm/lib/js_util_patch.dart`)
// over the handles of `dart:_js_helper`, with the same results and errors.

import "dart:_internal";
import "dart:_js_helper";
import "dart:js_interop" show JSAnyUtilityExtension, JSAny;
import "dart:collection";
import "dart:typed_data";

@patch
dynamic jsify(Object? object) {
  final convertedObjects = HashMap<Object?, Object?>.identity();
  Object? convert(Object? o) {
    if (convertedObjects.containsKey(o)) {
      return convertedObjects[o];
    }

    if (o == null ||
        o is num ||
        o is bool ||
        o is JSValue ||
        o is String ||
        (o is TypedData &&
            (o is Int8List ||
                o is Uint8List ||
                o is Uint8ClampedList ||
                o is Int16List ||
                o is Uint16List ||
                o is Int32List ||
                o is Uint32List ||
                o is Float32List ||
                o is Float64List ||
                o is ByteData)) ||
        o is ByteBuffer) {
      return jsifyRaw(o);
    }

    if (o is Map<Object?, Object?>) {
      final convertedMap = newObject<JSValue>();
      convertedObjects[o] = convertedMap;
      for (final key in o.keys) {
        final convertedKey = convert(key) as JSValue?;
        setPropertyRaw(convertedMap, convertedKey, convert(o[key]) as JSValue?);
      }
      return convertedMap;
    } else if (o is Iterable<Object?>) {
      final convertedIterable = _newArray();
      convertedObjects[o] = convertedIterable;
      for (final item in o) {
        callMethod(convertedIterable, 'push', [convert(item)]);
      }
      return convertedIterable;
    } else {
      // None of the objects left will require recursive conversions.
      return jsifyRaw(o);
    }
  }

  return convert(object);
}

@patch
Object get globalThis => globalThisRaw();

@patch
T newObject<T>() => newObjectRaw() as T;

JSValue _newArray() => newArrayRaw();

@patch
bool hasProperty(Object o, Object name) =>
    hasPropertyRaw(jsifyRaw(o), jsifyRaw(name));

@patch
T getProperty<T>(Object o, Object name) =>
    dartifyRaw(getPropertyRaw(jsifyRaw(o), jsifyRaw(name))) as T;

@patch
T setProperty<T>(Object o, Object name, T? value) =>
    dartifyRaw(setPropertyRaw(jsifyRaw(o), jsifyRaw(name), jsifyRaw(value)))
        as T;

@patch
T callMethod<T>(Object o, Object method, List<Object?> args) => dartifyRaw(
    callMethodVarArgsRaw(jsifyRaw(o), jsifyRaw(method), jsifyRaw(args))) as T;

@patch
bool instanceof(Object? o, Object type) => _instanceof(jsifyRaw(o), jsifyRaw(type));

@pragma('dartforge:js', '(o, t) => o instanceof t')
external bool _instanceof(JSValue? o, JSValue? t);

@patch
T callConstructor<T>(Object o, List<Object?>? args) =>
    dartifyRaw(callConstructorVarArgsRaw(jsifyRaw(o), jsifyRaw(args))) as T;

@patch
T add<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
T subtract<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
T multiply<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
T divide<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
T exponentiate<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
T modulo<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
bool equal<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
bool strictEqual<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
bool notEqual<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
bool strictNotEqual<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
bool greaterThan<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
bool greaterThanOrEqual<T>(Object? first, Object? second) =>
    throw 'unimplemented';

@patch
bool lessThan<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
bool lessThanOrEqual<T>(Object? first, Object? second) => throw 'unimplemented';

@patch
bool typeofEquals<T>(Object? o, String type) =>
    _typeofEquals(jsifyRaw(o), type);

@pragma('dartforge:js', '(o, t) => typeof o === t')
external bool _typeofEquals(JSValue? o, String t);

@patch
Object? objectGetPrototypeOf(Object? object) => throw 'unimplemented';

@patch
Object? get objectPrototype => throw 'unimplemented';

@patch
List<Object?> objectKeys(Object? o) => toDartList(_objectKeys(jsifyRaw(o)));

@pragma('dartforge:js', 'o => Object.keys(o)')
external JSValue? _objectKeys(JSValue? o);

@patch
Object? dartify(Object? object) {
  final convertedObjects = HashMap<Object?, Object?>.identity();
  Object? convert(Object? o) {
    if (convertedObjects.containsKey(o)) {
      return convertedObjects[o];
    }
    // Because [List] needs to be shallowly converted across the interop
    // boundary, we have to double check for the case where a shallowly
    // converted [List] is passed back into [dartify].
    if (o is List<Object?>) {
      final converted = <Object?>[];
      for (final item in o) {
        converted.add(convert(item));
      }
      return converted;
    }
    if (o is! JSValue) return o;
    JSValue ref = o;
    final refType = externRefType(ref);
    if (refType == ExternRefType.unknown && isJSSimpleObject(ref)) {
      final dartMap = <Object?, Object?>{};
      convertedObjects[o] = dartMap;
      // Keys will be a list of Dart [String]s.
      final keys = objectKeys(o);
      for (int i = 0; i < keys.length; i++) {
        final key = keys[i];
        if (key != null) {
          dartMap[key] = convert(getPropertyRaw(ref, jsifyRaw(key)));
        }
      }
      return dartMap;
    }
    if (refType == ExternRefType.array) {
      final dartList = <Object?>[];
      convertedObjects[o] = dartList;
      final length = getProperty<double>(o, 'length').toInt();
      for (int i = 0; i < length; i++) {
        dartList.add(convert(objectReadIndex(ref, i)));
      }
      return dartList;
    }
    return dartifyRaw(ref, refType);
  }

  return convert(object);
}

/// This will be lowered to a a call to `_wrapDartCallback`.
@patch
F allowInterop<F extends Function>(F f) => throw UnimplementedError();

@patch
Function allowInteropCaptureThis(Function f) => throw UnimplementedError();
