// The `dart:_js_helper` of DartForge's WebAssembly target (`dartforge_wasm`,
// docs/PLANO-WASM-MVP.md, "Desenho do interop JS"): the same library dart2wasm
// has (`_internal/wasm/lib/js_helper.dart`), over a linear memory.
//
// A JavaScript value is a handle of the adapter's table
// (runtime/wasm/dartforge_host_v1.mjs) that a `JSValue` owns. Each `external`
// with `@pragma('dartforge:js', '<code>')` is a snippet: the compiler writes its
// JavaScript into the module's companion `.mjs` and converts each value by its
// declared type (crates/emit_native/src/lower/js_interop.rs): `JSValue` and the
// JS types cross as handles, `double` and `int` as numbers, `bool` as 0/1 and
// `String` as a JavaScript string.

library dart._js_helper;

import 'dart:_internal';
import 'dart:_js_types' as js_types;
import 'dart:js_interop';
import 'dart:typed_data';

/// A JavaScript value: the handle of the adapter's table that this object
/// owns (0 is JavaScript `null`). When the object dies, its native finalizer
/// returns the handle to the table.
class JSValue {
  final int _ref;

  JSValue(this._ref) {
    _attach(this, _ref);
  }

  // This is currently only used in js_util.
  static JSValue? box(int ref) => ref == 0 ? null : JSValue(ref);

  static T boxT<T>(int ref) => box(ref) as T;

  // We need to handle the case of a nullable [JSValue] to match the semantics
  // of the JS backends.
  static int unbox(JSValue? v) => v == null ? 0 : v._ref;

  @override
  bool operator ==(Object that) => that is JSValue && areEqualInJS(this, that);

  // As in dart2wasm: JavaScript has no efficient hash code, so every
  // [JSValue] falls back to differentiation by equality.
  @override
  int get hashCode => 0;

  @override
  String toString() => stringify(this);
}

@pragma("vm:external-name", "DartForge_js_attach")
external void _attach(JSValue value, int handle);

/// The error a JavaScript exception becomes in Dart, as in dart2wasm: an
/// opaque object thrown from JavaScript.
class _JavaScriptError extends Error {
  _JavaScriptError();

  @override
  String toString() => "JavaScriptError";
}

// The runtime throws it when a snippet throws (crates/runtime/src/js_interop.rs).
@pragma("vm:entry-point")
Object _dartforgeJavaScriptError() => _JavaScriptError();

// Extension helpers to convert to the JavaScript value of a Dart value.

extension DoubleToJSValue on double? {
  JSValue? get toJSValue => this == null ? null : toJSNumber(this!);
}

extension StringToJSValue on String? {
  JSValue? get toJSValue => this == null ? null : jsStringFromDartString(this!);
}

extension ListOfObjectToJSValue on List<Object?>? {
  JSValue? get toJSValue => this == null ? null : jsArrayFromDartList(this!);
}

extension JSAnyToJSValue on JSAny? {
  JSValue? get toJSValue => this as JSValue?;
}

class JSArrayIteratorAdapter<T> implements Iterator<T> {
  final JSArray array;
  int index = -1;

  JSArrayIteratorAdapter(this.array);

  @override
  bool moveNext() {
    index++;
    int length = array.length;
    if (index > length) {
      throw 'Iterator out of bounds';
    }
    return index < length;
  }

  @override
  T get current => dartifyRaw(array[index] as JSValue?) as T;
}

/// [JSArrayIterableAdapter] lazily adapts a [JSArray] to Dart's [Iterable]
/// interface.
class JSArrayIterableAdapter<T> extends EfficientLengthIterable<T>
    implements HideEfficientLengthIterable<T> {
  final JSArray array;

  JSArrayIterableAdapter(this.array);

  @override
  Iterator<T> get iterator => JSArrayIteratorAdapter<T>(array);

  @override
  int get length => array.length;
}

JSValue? getConstructorString(String constructor) =>
    getPropertyRaw(globalThisRaw(), constructor.toJSValue);

@pragma('dartforge:js', 'o => o === undefined')
external bool isJSUndefined(JSValue? o);

@pragma('dartforge:js', "o => typeof o === 'boolean'")
external bool isJSBoolean(JSValue? o);

@pragma('dartforge:js', "o => typeof o === 'number'")
external bool isJSNumber(JSValue? o);

@pragma('dartforge:js', "o => typeof o === 'bigint'")
external bool isJSBigInt(JSValue? o);

@pragma('dartforge:js', "o => typeof o === 'string'")
external bool isJSString(JSValue? o);

@pragma('dartforge:js', "o => typeof o === 'symbol'")
external bool isJSSymbol(JSValue? o);

@pragma('dartforge:js', "o => typeof o === 'function'")
external bool isJSFunction(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Int8Array')
external bool isJSInt8Array(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Uint8Array')
external bool isJSUint8Array(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Uint8ClampedArray')
external bool isJSUint8ClampedArray(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Int16Array')
external bool isJSInt16Array(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Uint16Array')
external bool isJSUint16Array(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Int32Array')
external bool isJSInt32Array(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Uint32Array')
external bool isJSUint32Array(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Float32Array')
external bool isJSFloat32Array(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Float64Array')
external bool isJSFloat64Array(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof ArrayBuffer')
external bool isJSArrayBuffer(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof DataView')
external bool isJSDataView(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Array')
external bool isJSArray(JSValue? o);

@pragma('dartforge:js',
    "o => typeof o === 'function' && o[r.wrappedDartFunction] !== undefined")
external bool isJSWrappedDartFunction(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof Object')
external bool isJSObject(JSValue? o);

@pragma('dartforge:js', '''o => {
            const proto = Object.getPrototypeOf(o);
            return proto === Object.prototype || proto === null;
          }''')
external bool isJSSimpleObject(JSValue? o);

@pragma('dartforge:js', 'o => o instanceof RegExp')
external bool isJSRegExp(JSValue? o);

@pragma('dartforge:js', '(l, r) => l === r')
external bool areEqualInJS(JSValue? l, JSValue? r);

// The JS runtime will run helpful conversion routines between handles and
// bool / double.
@pragma('dartforge:js', 'o => o')
external double toDartNumber(JSValue? o);

@pragma('dartforge:js', 'o => o')
external JSValue toJSNumber(double o);

@pragma('dartforge:js', 'o => o')
external bool toDartBool(JSValue? o);

@pragma('dartforge:js', 'b => !!b')
external JSValue toJSBoolean(bool b);

@pragma('dartforge:js', 'o => o.length')
external int objectLength(JSValue? o);

@pragma('dartforge:js', 'o => o.byteLength')
external int byteLength(JSValue? o);

@pragma('dartforge:js', '(o, i) => o.getUint8(i)')
external int dataViewGetUint8(JSValue? o, int i);

@pragma('dartforge:js', '(o, i) => o[i]')
external JSValue? objectReadIndex(JSValue? o, int index);

/// The JavaScript string of a Dart string.
@pragma('dartforge:js', 's => s')
external JSValue jsStringFromDartString(String s);

/// The Dart string of a JavaScript string.
@pragma('dartforge:js', 's => s')
external String jsStringToDartString(JSValue s);

@pragma('dartforge:js', '() => ({})')
external JSValue newObjectRaw();

@pragma('dartforge:js', '() => []')
external JSValue newArrayRaw();

@pragma('dartforge:js', 'l => new Array(l)')
external JSValue newArrayFromLengthRaw(int length);

@pragma('dartforge:js', '() => globalThis')
external JSValue globalThisRaw();

// Apply bind to the constructor. We pass `null` as the first argument to
// `bind.apply` because this is `bind`'s unused context argument (`new` will
// explicitly create a new context).
@pragma('dartforge:js', '''(constructor, args) => {
      const factoryFunction = constructor.bind.apply(
          constructor, [null, ...args]);
      return new factoryFunction();
    }''')
external JSValue? callConstructorVarArgsRaw(JSValue? o, JSValue? args);

@pragma('dartforge:js', '(o, p) => p in o')
external bool hasPropertyRaw(JSValue? o, JSValue? p);

@pragma('dartforge:js', '(o, p) => o[p]')
external JSValue? getPropertyRaw(JSValue? o, JSValue? p);

@pragma('dartforge:js', '(o, p, v) => o[p] = v')
external JSValue? setPropertyRaw(JSValue? o, JSValue? p, JSValue? v);

@pragma('dartforge:js', '(o, m, a) => o[m].apply(o, a)')
external JSValue? callMethodVarArgsRaw(JSValue? o, JSValue? method, JSValue? args);

@pragma('dartforge:js', 'o => typeof o')
external String typeof(JSValue? object);

@pragma('dartforge:js', 'o => String(o)')
external String stringify(JSValue? object);

@pragma('dartforge:js', '(p, s, f) => p.then(s, f)')
external void promiseThen(JSValue? promise, JSValue? successFunc, JSValue? failureFunc);

/// Returns the JS constructor object for a given [String].
JSValue? getConstructorRaw(String name) =>
    getPropertyRaw(globalThisRaw(), name.toJSValue);

/// A JavaScript array with the values of [l], converted by [jsifyRaw].
JSValue jsArrayFromDartList(List<Object?> l) {
  final int length = l.length;
  final JSValue array = newArrayFromLengthRaw(length);
  for (int i = 0; i < length; i++) {
    _arraySet(array, i, jsifyRaw(l[i]));
  }
  return array;
}

@pragma('dartforge:js', '(a, i, v) => { a[i] = v; }')
external void _arraySet(JSValue a, int i, JSValue? v);

JSValue? jsifyRaw(Object? o) {
  if (o == null) return null;
  if (o is bool) return toJSBoolean(o);
  if (o is num) return toJSNumber(o.toDouble());
  if (o is JSValue) return o;
  if (o is String) return jsStringFromDartString(o);
  if (o is List<Object?>) return jsArrayFromDartList(o);
  return null;
}

/// Constants that represent the possible types of a JavaScript value that can
/// then be used in a [dartifyRaw] call.
///
/// The values within this class should correspond to the values returned by
/// [externRefType] and should be updated if that function is updated.
class ExternRefType {
  static const int null_ = 0;
  static const int undefined = 1;
  static const int boolean = 2;
  static const int number = 3;
  static const int string = 4;
  static const int array = 5;
  static const int int8Array = 6;
  static const int uint8Array = 7;
  static const int uint8ClampedArray = 8;
  static const int int16Array = 9;
  static const int uint16Array = 10;
  static const int int32Array = 11;
  static const int uint32Array = 12;
  static const int float32Array = 13;
  static const int float64Array = 14;
  static const int dataView = 15;
  static const int arrayBuffer = 16;
  static const int unknown = 17;
}

/// Returns an integer representing the type of [ref] that corresponds to one of
/// the constant values in [ExternRefType].
int externRefType(JSValue? ref) => ref == null ? ExternRefType.null_ : _externRefType(ref);

@pragma('dartforge:js', '''o => {
    if (o === undefined) return 1;
    var type = typeof o;
    if (type === 'boolean') return 2;
    if (type === 'number') return 3;
    if (type === 'string') return 4;
    if (o instanceof Array) return 5;
    if (ArrayBuffer.isView(o)) {
      if (o instanceof Int8Array) return 6;
      if (o instanceof Uint8Array) return 7;
      if (o instanceof Uint8ClampedArray) return 8;
      if (o instanceof Int16Array) return 9;
      if (o instanceof Uint16Array) return 10;
      if (o instanceof Int32Array) return 11;
      if (o instanceof Uint32Array) return 12;
      if (o instanceof Float32Array) return 13;
      if (o instanceof Float64Array) return 14;
      if (o instanceof DataView) return 15;
    }
    if (o instanceof ArrayBuffer) return 16;
    return 17;
  }''')
external int _externRefType(JSValue o);

/// Non-recursively converts [ref] from a JS value to a Dart value for some JS
/// types.
///
/// If [refType] is not null, it is treated as one of the values from
/// [ExternRefType]. Otherwise, this method calls [externRefType] to determine
/// the right [ExternRefType].
Object? dartifyRaw(JSValue? ref, [int? refType]) {
  refType ??= externRefType(ref);
  return switch (refType) {
    ExternRefType.null_ || ExternRefType.undefined => null,
    ExternRefType.boolean => toDartBool(ref),
    ExternRefType.number => toDartNumber(ref),
    ExternRefType.string => jsStringToDartString(ref!),
    ExternRefType.array => toDartList(ref),
    _ => ref,
  };
}

List<JSAny?> toDartListJSAny(JSValue? ref) => List<JSAny?>.generate(
    objectLength(ref), (int n) => objectReadIndex(ref, n) as JSAny?);

List<Object?> toDartList(JSValue? ref) => List<Object?>.generate(
    objectLength(ref), (int n) => dartifyRaw(objectReadIndex(ref, n)));

// The conversions of the interop members' values
// (crates/emit_native/src/lower/js_members.rs), with dart2wasm 3.6's results and
// errors: a `num` argument crosses as its `double`; a result comes as the
// handle the trampoline returned and becomes the declared type.

double jsArgumentNum(num n) => n.toDouble();

// A non-nullable JS type: the null check of dart2wasm.
JSValue jsResultValue(int ref) => JSValue.box(ref)!;

int? _toDartNullableInt(JSValue? ref) {
  final number = dartifyRaw(ref) as double?;
  if (number == null) return null;
  final intVal = number.toInt();
  if (number == intVal) return intVal;
  throw 'Expected integer value, but was not integer.';
}

int jsResultInt(int ref) => _toDartNullableInt(JSValue.box(ref)) as int;

int? jsResultNullableInt(int ref) => _toDartNullableInt(JSValue.box(ref));

double jsResultDouble(int ref) => dartifyRaw(JSValue.box(ref)) as double;

double? jsResultNullableDouble(int ref) => dartifyRaw(JSValue.box(ref)) as double?;

num jsResultNum(int ref) => dartifyRaw(JSValue.box(ref)) as num;

num? jsResultNullableNum(int ref) => dartifyRaw(JSValue.box(ref)) as num?;

bool jsResultBool(int ref) => dartifyRaw(JSValue.box(ref)) as bool;

bool? jsResultNullableBool(int ref) => dartifyRaw(JSValue.box(ref)) as bool?;

String jsResultString(int ref) => dartifyRaw(JSValue.box(ref)) as String;

String? jsResultNullableString(int ref) => dartifyRaw(JSValue.box(ref)) as String?;

Object? jsResultAny(int ref) => dartifyRaw(JSValue.box(ref));
