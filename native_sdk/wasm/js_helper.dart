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
    "o => typeof o === 'function' && o[r.wrappedDartFunction] === true")
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
  if (o is js_types.JSArrayBase) {
    if (o is js_types.JSInt8ArrayImpl) return o.toJSArrayJSValue();
    if (o is js_types.JSUint8ArrayImpl) return o.toJSArrayJSValue();
    if (o is js_types.JSUint8ClampedArrayImpl) return o.toJSArrayJSValue();
    if (o is js_types.JSInt16ArrayImpl) return o.toJSArrayJSValue();
    if (o is js_types.JSUint16ArrayImpl) return o.toJSArrayJSValue();
    if (o is js_types.JSInt32ArrayImpl) return o.toJSArrayJSValue();
    if (o is js_types.JSUint32ArrayImpl) return o.toJSArrayJSValue();
    if (o is js_types.JSFloat32ArrayImpl) return o.toJSArrayJSValue();
    if (o is js_types.JSFloat64ArrayImpl) return o.toJSArrayJSValue();
  } else if (o is TypedData) {
    if (o is Int8List) return jsInt8ArrayFromDartInt8List(o);
    if (o is Uint8List) return jsUint8ArrayFromDartUint8List(o);
    if (o is Uint8ClampedList) {
      return jsUint8ClampedArrayFromDartUint8ClampedList(o);
    }
    if (o is Int16List) return jsInt16ArrayFromDartInt16List(o);
    if (o is Uint16List) return jsUint16ArrayFromDartUint16List(o);
    if (o is Int32List) return jsInt32ArrayFromDartInt32List(o);
    if (o is Uint32List) return jsUint32ArrayFromDartUint32List(o);
    if (o is Float32List) return jsFloat32ArrayFromDartFloat32List(o);
    if (o is Float64List) return jsFloat64ArrayFromDartFloat64List(o);
    if (o is js_types.JSDataViewImpl) return o.toJSValue;
    if (o is ByteData) return jsDataViewFromDartByteData(o, o.lengthInBytes);
  } else if (o is List<Object?>) {
    return jsArrayFromDartList(o);
  } else if (o is ByteBuffer) {
    if (o is js_types.JSArrayBufferImpl) return o.toJSValue;
    return jsArrayBufferFromDartByteBuffer(o);
  }
  return jsObjectFromDartObject(o);
}

// The JavaScript copy of a typed list of the runtime: its bytes, read from the
// linear memory (`r.memory`) in one slice; an object that is not one of the
// runtime's (a view the program wrote) goes element by element.

@pragma("vm:external-name", "DartForge_js_typed_data_address")
external int _typedDataAddress(TypedData data);

@pragma('dartforge:js', '(p, n) => new Int8Array(r.memory.buffer.slice(p, p + n))')
external JSValue _int8ArrayOf(int address, int bytes);

@pragma('dartforge:js', '(p, n) => new Uint8Array(r.memory.buffer.slice(p, p + n))')
external JSValue _uint8ArrayOf(int address, int bytes);

@pragma('dartforge:js',
    '(p, n) => new Uint8ClampedArray(r.memory.buffer.slice(p, p + n))')
external JSValue _uint8ClampedArrayOf(int address, int bytes);

@pragma('dartforge:js', '(p, n) => new Int16Array(r.memory.buffer.slice(p, p + n))')
external JSValue _int16ArrayOf(int address, int bytes);

@pragma('dartforge:js', '(p, n) => new Uint16Array(r.memory.buffer.slice(p, p + n))')
external JSValue _uint16ArrayOf(int address, int bytes);

@pragma('dartforge:js', '(p, n) => new Int32Array(r.memory.buffer.slice(p, p + n))')
external JSValue _int32ArrayOf(int address, int bytes);

@pragma('dartforge:js', '(p, n) => new Uint32Array(r.memory.buffer.slice(p, p + n))')
external JSValue _uint32ArrayOf(int address, int bytes);

@pragma('dartforge:js', '(p, n) => new Float32Array(r.memory.buffer.slice(p, p + n))')
external JSValue _float32ArrayOf(int address, int bytes);

@pragma('dartforge:js', '(p, n) => new Float64Array(r.memory.buffer.slice(p, p + n))')
external JSValue _float64ArrayOf(int address, int bytes);

@pragma('dartforge:js', '(p, n) => new DataView(r.memory.buffer.slice(p, p + n))')
external JSValue _dataViewOf(int address, int bytes);

@pragma('dartforge:js', '(k, n) => new globalThis[k](n)')
external JSValue _newTypedArray(String constructor, int length);

@pragma('dartforge:js', '(a, i, v) => { a[i] = v; }')
external void _setNumber(JSValue a, int i, double v);

JSValue _copied(TypedData l, JSValue Function(int, int) slice, String constructor) {
  final address = _typedDataAddress(l);
  if (address >= 0) return slice(address, l.lengthInBytes);
  final list = l as List<num>;
  final copy = _newTypedArray(constructor, list.length);
  for (var i = 0; i < list.length; i++) {
    _setNumber(copy, i, list[i].toDouble());
  }
  return copy;
}

JSValue jsInt8ArrayFromDartInt8List(Int8List l) =>
    _copied(l, _int8ArrayOf, 'Int8Array');

JSValue jsUint8ArrayFromDartUint8List(Uint8List l) =>
    _copied(l, _uint8ArrayOf, 'Uint8Array');

JSValue jsUint8ClampedArrayFromDartUint8ClampedList(Uint8ClampedList l) =>
    _copied(l, _uint8ClampedArrayOf, 'Uint8ClampedArray');

JSValue jsInt16ArrayFromDartInt16List(Int16List l) =>
    _copied(l, _int16ArrayOf, 'Int16Array');

JSValue jsUint16ArrayFromDartUint16List(Uint16List l) =>
    _copied(l, _uint16ArrayOf, 'Uint16Array');

JSValue jsInt32ArrayFromDartInt32List(Int32List l) =>
    _copied(l, _int32ArrayOf, 'Int32Array');

JSValue jsUint32ArrayFromDartUint32List(Uint32List l) =>
    _copied(l, _uint32ArrayOf, 'Uint32Array');

JSValue jsFloat32ArrayFromDartFloat32List(Float32List l) =>
    _copied(l, _float32ArrayOf, 'Float32Array');

JSValue jsFloat64ArrayFromDartFloat64List(Float64List l) =>
    _copied(l, _float64ArrayOf, 'Float64Array');

JSValue jsDataViewFromDartByteData(ByteData data, int length) {
  final address = _typedDataAddress(data);
  if (address >= 0) return _dataViewOf(address, length);
  final bytes = Uint8List(length);
  for (var i = 0; i < length; i++) {
    bytes[i] = data.getUint8(i);
  }
  return _dataViewOf(_typedDataAddress(bytes), length);
}

JSValue? jsArrayBufferFromDartByteBuffer(ByteBuffer buffer) {
  ByteData byteData = ByteData.view(buffer);
  JSValue dataView = jsDataViewFromDartByteData(byteData, byteData.lengthInBytes);
  return getPropertyRaw(dataView, 'buffer'.toJSValue);
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
    ExternRefType.int8Array => js_types.JSInt8ArrayImpl.fromJSArray(ref),
    ExternRefType.uint8Array => js_types.JSUint8ArrayImpl.fromJSArray(ref),
    ExternRefType.uint8ClampedArray =>
      js_types.JSUint8ClampedArrayImpl.fromJSArray(ref),
    ExternRefType.int16Array => js_types.JSInt16ArrayImpl.fromJSArray(ref),
    ExternRefType.uint16Array => js_types.JSUint16ArrayImpl.fromJSArray(ref),
    ExternRefType.int32Array => js_types.JSInt32ArrayImpl.fromJSArray(ref),
    ExternRefType.uint32Array => js_types.JSUint32ArrayImpl.fromJSArray(ref),
    ExternRefType.float32Array => js_types.JSFloat32ArrayImpl.fromJSArray(ref),
    ExternRefType.float64Array => js_types.JSFloat64ArrayImpl.fromJSArray(ref),
    ExternRefType.arrayBuffer => js_types.JSArrayBufferImpl.fromRef(ref),
    ExternRefType.dataView => js_types.JSDataViewImpl.fromRef(ref),
    _ => isJSWrappedDartFunction(ref)
        ? jsExportedDartFunctionToDart(ref!)
        : _isDartRef(ref)
            ? jsObjectToDartObject(ref)
            : ref,
  };
}

bool _isDartRef(JSValue? ref) => _dartRefId(ref) >= 0;

JSArray<T> toJSArray<T extends JSAny?>(List<T> list) {
  int length = list.length;
  JSArray<T> result = JSArray<T>.withLength(length);
  for (int i = 0; i < length; i++) {
    result[i] = list[i];
  }
  return result;
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

// Dart objects in JavaScript: the id of a global root of the runtime
// (crates/runtime/src/js_interop.rs) inside a `DartRef` of the adapter, the
// same for the same object while JavaScript keeps it.

@pragma("vm:external-name", "DartForge_js_dart_ref")
external int _dartRef(Object object);

@pragma("vm:external-name", "DartForge_js_dart_object")
external Object _dartObject(int id);

@pragma('dartforge:js', 'id => r.dartRef(id)')
external JSValue _dartRefValue(int id);

@pragma('dartforge:js', 'o => o instanceof r.DartRef ? o.id : -1')
external int _dartRefId(JSValue? o);

/// The JavaScript value of a Dart object (dart2wasm's `externalize`).
JSValue jsObjectFromDartObject(Object object) => _dartRefValue(_dartRef(object));

/// The Dart object of a JavaScript value [jsObjectFromDartObject] made.
Object jsObjectToDartObject(JSValue? ref) {
  final id = _dartRefId(ref);
  if (id < 0) {
    throw 'Expected a Dart object, but got a JS value instead.';
  }
  return _dartObject(id);
}

// `Function.toJS` (crates/emit_native/src/lower/js_members.rs): the compiler
// describes the static type of the function and the wrapper of the adapter calls
// it back through `_jsCall`, as dart2wasm's trampolines: as many
// arguments as JavaScript passed, between the required and the positional
// count, each converted by its type (`kinds`: `r`/`R` a nullable/non-nullable
// JS type, `i`/`I` int, `d`/`D` double, `n`/`N` num, `b`/`B` bool, `s`/`S`
// String, `o` anything else), and the result by its type (`v` void, `r` a JS
// type, `j` anything else).

final class _JSCallback {
  final Function function;
  final String kinds;
  final String result;
  final int positional;
  final int required;

  _JSCallback(
      this.function, this.kinds, this.result, this.positional, this.required);
}

JSFunction jsFunctionToJS(Function function, String kinds, String result,
    int positional, int required, bool captureThis) {
  final callback = _JSCallback(function, kinds, result, positional, required);
  return _wrapDartFunction(
      jsObjectFromDartObject(callback),
      captureThis ? positional - 1 : positional,
      captureThis,
      _jsCallAddress()) as JSFunction;
}

@pragma('dartforge:js',
    '(f, n, t, p) => r.wrapDartFunction(f, n, t !== 0, p)')
external JSValue _wrapDartFunction(
    JSValue f, int length, bool captureThis, int trampoline);

/// The address of [_jsCall], which the runtime calls for the wrapper
/// (`dartforge_js_call`).
@pragma('dartforge:address-of', '_jsCall')
external int _jsCallAddress();

@pragma('dartforge:js', 'f => f.dartFunction')
external JSValue _dartFunctionOf(JSValue f);

// `createJSInteropWrapper` (dart2wasm's `_createExport`): the object, a
// property per exported method and an accessor property per exported getter
// and setter.

/// The object of an export: `{}`, or an object with the prototype `proto`.
@pragma('dartforge:js', 'p => p == null ? {} : Object.create(p)')
external JSValue jsExportObject(JSValue? proto);

/// `o[name] = f`: an exported method.
@pragma('dartforge:js', '(o, n, f) => { o[n] = f; }')
external void jsExportProperty(JSValue o, String name, JSValue f);

/// `Object.defineProperty(o, name, {get, set})`: an exported getter and setter.
@pragma('dartforge:js', '''(o, n, g, s) => {
  const d = {};
  if (g != null) d.get = g;
  if (s != null) d.set = s;
  Object.defineProperty(o, n, d);
}''')
external void jsExportAccessor(
    JSValue o, String name, JSValue? getter, JSValue? setter);

/// The Dart function a [jsFunctionToJS] wrapper calls.
Function jsExportedDartFunctionToDart(JSValue ref) {
  if (!isJSWrappedDartFunction(ref)) {
    throw 'Expected JS wrapped function, but got type ${typeof(ref)}.';
  }
  return (jsObjectToDartObject(_dartFunctionOf(ref)) as _JSCallback).function;
}

@pragma('dartforge:js', 'i => r.argument(i)')
external JSValue? _callbackArgument(int i);

Object? _convertArgument(int kind, JSValue? v) => switch (kind) {
      0x72 /* r */ => v,
      0x52 /* R */ => v!,
      0x69 /* i */ => _toDartNullableInt(v) as int,
      0x49 /* I */ => _toDartNullableInt(v),
      0x64 /* d */ => dartifyRaw(v) as double,
      0x44 /* D */ => dartifyRaw(v) as double?,
      0x6e /* n */ => dartifyRaw(v) as num,
      0x4e /* N */ => dartifyRaw(v) as num?,
      0x62 /* b */ => dartifyRaw(v) as bool,
      0x42 /* B */ => dartifyRaw(v) as bool?,
      0x73 /* s */ => dartifyRaw(v) as String,
      0x53 /* S */ => dartifyRaw(v) as String?,
      _ => dartifyRaw(v),
    };

// The runtime calls it for the adapter (`dartforge_js_call`): the handle of the
// result, which the adapter reads before anything else runs.
int _jsCall(Object callback, int argc) {
  final c = callback as _JSCallback;
  final int count;
  if (argc >= c.positional) {
    count = c.positional;
  } else if (argc >= c.required) {
    count = argc;
  } else {
    throw 'Too few arguments passed. '
        'Expected ${c.required} or more, got $argc instead.';
  }
  final arguments = List<Object?>.filled(count, null);
  for (var i = 0; i < count; i++) {
    arguments[i] =
        _convertArgument(c.kinds.codeUnitAt(i), _callbackArgument(i));
  }
  final r = Function.apply(c.function, arguments);
  return switch (c.result) {
    'v' => 0,
    'r' => JSValue.unbox(r as JSValue?),
    _ => JSValue.unbox(jsifyRaw(r)),
  };
}

// `v.isA<T>()` (crates/emit_native/src/lower/js_members.rs): `mode` 0 is any
// value but `null`, 1 a `typeof` of `name`, 2 an `instanceof` of the
// constructor `name` names from `globalThis` (`instanceOfString`), 3 a typed
// array (`TypedArray` is not a global: the prototype the typed arrays share).
bool jsIsA(JSValue? v, bool nullable, int mode, String name) {
  if (v == null) return nullable;
  return switch (mode) {
    0 => true,
    1 => _typeofIs(v, name),
    3 => _isTypedArray(v),
    _ => _instanceOfString(v, name),
  };
}

@pragma('dartforge:js', 'o => o instanceof Object.getPrototypeOf(Int8Array)')
external bool _isTypedArray(JSValue o);

@pragma('dartforge:js', '(o, t) => typeof o === t')
external bool _typeofIs(JSValue o, String type);

@pragma('dartforge:js', '''(o, n) => {
    if (n.length === 0) return false;
    let c = globalThis;
    for (const p of n.split('.')) {
      c = c[p];
      if (c == null) return false;
    }
    return o instanceof c;
  }''')
external bool _instanceOfString(JSValue o, String name);
