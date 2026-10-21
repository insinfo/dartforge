// The `dart:js_interop` patch of DartForge's WebAssembly target
// (`dartforge_wasm`): dart2wasm's (`_internal/wasm/lib/js_interop_patch.dart`)
// over the handles of `dart:_js_helper`, with the same results and errors.

import 'dart:_internal' show patch;
import 'dart:_js_helper' as js_helper;
import 'dart:_js_annotations' show JSExport;
import 'dart:_js_helper' hide JS;
import 'dart:_js_types' as js_types;
import 'dart:async' show Completer;
import 'dart:js_interop_unsafe' as unsafe;
import 'dart:js_util' as js_util;
import 'dart:typed_data';

@patch
js_types.JSObjectRepType _createObjectLiteral() => js_helper.newObjectRaw();

// This should match the global context we use in our static interop lowerings.
@patch
JSObject get globalContext => js_util.globalThis as JSObject;

// Helper for working with the JSAny? top type in a backend agnostic way.
@patch
extension NullableUndefineableJSAnyExtension on JSAny? {
  @patch
  bool get isUndefined =>
      throw UnimplementedError("JS 'null' and 'undefined' are internalized as "
          "Dart null in dart2wasm. As such, they can not be differentiated and "
          "this API should not be used when compiling to Wasm.");

  @patch
  bool get isNull =>
      throw UnimplementedError("JS 'null' and 'undefined' are internalized as "
          "Dart null in dart2wasm. As such, they can not be differentiated and "
          "this API should not be used when compiling to Wasm.");
}

@patch
extension JSAnyUtilityExtension on JSAny? {
  @patch
  bool typeofEquals(String type) => _typeofEquals(toJSValue, type);

  @patch
  bool instanceof(JSFunction constructor) =>
      _instanceof(toJSValue, constructor.toJSValue);

  @patch
  bool isA<T extends JSAny?>() => throw UnimplementedError(
      "This should never be called. Calls to 'isA' should have been "
      'transformed by the interop transformer.');

  @patch
  Object? dartify() => js_util.dartify(this);
}

@pragma('dartforge:js', '(o, t) => typeof o === t')
external bool _typeofEquals(JSValue? o, String t);

@pragma('dartforge:js', '(o, c) => o instanceof c')
external bool _instanceof(JSValue? o, JSValue? c);

// Utility extensions for Object?.
@patch
extension NullableObjectUtilExtension on Object? {
  @patch
  JSAny? jsify() => js_util.jsify(this);
}

// -----------------------------------------------------------------------------
// JSExportedDartFunction <-> Function
@patch
extension JSExportedDartFunctionToFunction on JSExportedDartFunction {
  @patch
  Function get toDart => jsExportedDartFunctionToDart(toJSValue!);
}

@patch
extension FunctionToJSExportedDartFunction on Function {
  @patch
  JSExportedDartFunction get toJS => throw UnimplementedError(
      "This should never be called. Calls to 'toJS' should have been "
      'transformed by the interop transformer.');

  @patch
  JSExportedDartFunction get toJSCaptureThis => throw UnimplementedError(
      "'toJSCaptureThis' should never directly be called. Calls to "
      "'toJSCaptureThis' should have been transformed by the interop "
      'transformer.');
}

// Embedded global property for wrapped Dart objects passed via JS interop.
//
// This is a Symbol so that different Dart applications don't share Dart
// objects from different Dart runtimes. We expect all JSBoxedDartObjects to
// have this Symbol.
final JSValue _jsBoxedDartObjectProperty = _newBoxSymbol();

@pragma('dartforge:js', '() => Symbol("jsBoxedDartObjectProperty")')
external JSValue _newBoxSymbol();

@pragma('dartforge:js', '(o, s) => o[s]')
external JSValue? _getSymbolProperty(JSValue? o, JSValue? s);

@pragma('dartforge:js', '(o, s, v) => o[s] = v')
external void _setSymbolProperty(JSValue? o, JSValue? s, JSValue? v);

// -----------------------------------------------------------------------------
// JSBoxedDartObject <-> Object
@patch
extension JSBoxedDartObjectToObject on JSBoxedDartObject {
  @patch
  Object get toDart {
    final val = _getSymbolProperty(toJSValue, _jsBoxedDartObjectProperty);
    if (val == null) {
      throw 'Expected a wrapped Dart object, but got a JS object or a wrapped '
          'Dart object from a separate runtime instead.';
    }
    return jsObjectToDartObject(val);
  }
}

@patch
extension ObjectToJSBoxedDartObject on Object {
  @patch
  JSBoxedDartObject get toJSBox {
    if (this is JSValue) {
      throw 'Attempting to box non-Dart object.';
    }
    final box = JSObject();
    _setSymbolProperty(
        box.toJSValue, _jsBoxedDartObjectProperty, jsObjectFromDartObject(this));
    return JSBoxedDartObject._(box._jsObject);
  }
}

// -----------------------------------------------------------------------------
// ExternalDartReference <-> Object
@patch
extension ExternalDartReferenceToObject<T extends Object?>
    on ExternalDartReference<T> {
  @patch
  T get toDartObject {
    final t = this._externalDartReference;
    return (t == null ? null : jsObjectToDartObject(t)) as T;
  }
}

@patch
extension ObjectToExternalDartReference<T extends Object?> on T {
  @patch
  ExternalDartReference<T> get toExternalReference {
    final t = this;
    return ExternalDartReference<T>._(
        t == null ? null : jsObjectFromDartObject(t));
  }
}

// JSPromise -> Future
@patch
extension JSPromiseToFuture<T extends JSAny?> on JSPromise<T> {
  @patch
  Future<T> get toDart {
    final completer = Completer<T>();
    final success = (JSAny? r) {
      // Note that we explicitly type the parameter as `JSAny?` instead of `T`.
      // This is because if there's a `TypeError` with the cast, we want to
      // bubble that up through the completer, so we end up doing a try-catch
      // here to do so.
      try {
        final value = r as T;
        completer.complete(value);
      } catch (e) {
        completer.completeError(e);
      }
    }.toJS;
    final error = (JSAny? e) {
      if (e == null) {
        // Note that we pass false as a default. It's not currently possible to
        // be able to differentiate between null and undefined.
        completer.completeError(js_util.NullRejectionException(false));
        return;
      }
      completer.completeError(e);
    }.toJS;
    promiseThen(toJSValue, success.toJSValue, error.toJSValue);
    return completer.future;
  }
}

// -----------------------------------------------------------------------------
// JSArrayBuffer <-> ByteBuffer
@patch
extension JSArrayBufferToByteBuffer on JSArrayBuffer {
  @patch
  ByteBuffer get toDart => js_types.JSArrayBufferImpl.fromRef(toJSValue);
}

@patch
extension ByteBufferToJSArrayBuffer on ByteBuffer {
  @patch
  JSArrayBuffer get toJS {
    final t = this;
    return JSArrayBuffer._(t is js_types.JSArrayBufferImpl
        ? t.toJSValue!
        : jsArrayBufferFromDartByteBuffer(t)!);
  }
}

// -----------------------------------------------------------------------------
// JSDataView <-> ByteData
@patch
extension JSDataViewToByteData on JSDataView {
  @patch
  ByteData get toDart => js_types.JSDataViewImpl.fromRef(toJSValue);
}

@patch
extension ByteDataToJSDataView on ByteData {
  @patch
  JSDataView get toJS {
    final t = this;
    return JSDataView._(t is js_types.JSDataViewImpl
        ? t.toJSValue!
        : jsDataViewFromDartByteData(t, lengthInBytes));
  }
}

// -----------------------------------------------------------------------------
// JSInt8Array <-> Int8List
@patch
extension JSInt8ArrayToInt8List on JSInt8Array {
  @patch
  Int8List get toDart => js_types.JSInt8ArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Int8ListToJSInt8Array on Int8List {
  @patch
  JSInt8Array get toJS {
    final t = this;
    return JSInt8Array._(t is js_types.JSInt8ArrayImpl
        ? t.toJSArrayJSValue()!
        : jsInt8ArrayFromDartInt8List(t));
  }
}

// -----------------------------------------------------------------------------
// JSUint8Array <-> Uint8List
@patch
extension JSUint8ArrayToUint8List on JSUint8Array {
  @patch
  Uint8List get toDart => js_types.JSUint8ArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Uint8ListToJSUint8Array on Uint8List {
  @patch
  JSUint8Array get toJS {
    final t = this;
    return JSUint8Array._(t is js_types.JSUint8ArrayImpl
        ? t.toJSArrayJSValue()!
        : jsUint8ArrayFromDartUint8List(t));
  }
}

// -----------------------------------------------------------------------------
// JSUint8ClampedArray <-> Uint8ClampedList
@patch
extension JSUint8ClampedArrayToUint8ClampedList on JSUint8ClampedArray {
  @patch
  Uint8ClampedList get toDart =>
      js_types.JSUint8ClampedArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Uint8ClampedListToJSUint8ClampedArray on Uint8ClampedList {
  @patch
  JSUint8ClampedArray get toJS {
    final t = this;
    return JSUint8ClampedArray._(t is js_types.JSUint8ClampedArrayImpl
        ? t.toJSArrayJSValue()!
        : jsUint8ClampedArrayFromDartUint8ClampedList(t));
  }
}

// -----------------------------------------------------------------------------
// JSInt16Array <-> Int16List
@patch
extension JSInt16ArrayToInt16List on JSInt16Array {
  @patch
  Int16List get toDart => js_types.JSInt16ArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Int16ListToJSInt16Array on Int16List {
  @patch
  JSInt16Array get toJS {
    final t = this;
    return JSInt16Array._(t is js_types.JSInt16ArrayImpl
        ? t.toJSArrayJSValue()!
        : jsInt16ArrayFromDartInt16List(t));
  }
}

// -----------------------------------------------------------------------------
// JSUint16Array <-> Uint16List
@patch
extension JSUint16ArrayToInt16List on JSUint16Array {
  @patch
  Uint16List get toDart => js_types.JSUint16ArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Uint16ListToJSInt16Array on Uint16List {
  @patch
  JSUint16Array get toJS {
    final t = this;
    return JSUint16Array._(t is js_types.JSUint16ArrayImpl
        ? t.toJSArrayJSValue()!
        : jsUint16ArrayFromDartUint16List(t));
  }
}

// -----------------------------------------------------------------------------
// JSInt32Array <-> Int32List
@patch
extension JSInt32ArrayToInt32List on JSInt32Array {
  @patch
  Int32List get toDart => js_types.JSInt32ArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Int32ListToJSInt32Array on Int32List {
  @patch
  JSInt32Array get toJS {
    final t = this;
    return JSInt32Array._(t is js_types.JSInt32ArrayImpl
        ? t.toJSArrayJSValue()!
        : jsInt32ArrayFromDartInt32List(t));
  }
}

// -----------------------------------------------------------------------------
// JSUint32Array <-> Uint32List
@patch
extension JSUint32ArrayToUint32List on JSUint32Array {
  @patch
  Uint32List get toDart => js_types.JSUint32ArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Uint32ListToJSUint32Array on Uint32List {
  @patch
  JSUint32Array get toJS {
    final t = this;
    return JSUint32Array._(t is js_types.JSUint32ArrayImpl
        ? t.toJSArrayJSValue()!
        : jsUint32ArrayFromDartUint32List(t));
  }
}

// -----------------------------------------------------------------------------
// JSFloat32Array <-> Float32List
@patch
extension JSFloat32ArrayToFloat32List on JSFloat32Array {
  @patch
  Float32List get toDart =>
      js_types.JSFloat32ArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Float32ListToJSFloat32Array on Float32List {
  @patch
  JSFloat32Array get toJS {
    final t = this;
    return JSFloat32Array._(t is js_types.JSFloat32ArrayImpl
        ? t.toJSArrayJSValue()!
        : jsFloat32ArrayFromDartFloat32List(t));
  }
}

// -----------------------------------------------------------------------------
// JSFloat64Array <-> Float64List
@patch
extension JSFloat64ArrayToFloat64List on JSFloat64Array {
  @patch
  Float64List get toDart =>
      js_types.JSFloat64ArrayImpl.fromJSArray(toJSValue);
}

@patch
extension Float64ListToJSFloat64Array on Float64List {
  @patch
  JSFloat64Array get toJS {
    final t = this;
    return JSFloat64Array._(t is js_types.JSFloat64ArrayImpl
        ? t.toJSArrayJSValue()!
        : jsFloat64ArrayFromDartFloat64List(t));
  }
}

// -----------------------------------------------------------------------------
// JSArray <-> List
@patch
extension JSArrayToList<T extends JSAny?> on JSArray<T> {
  @patch
  List<T> get toDart => js_types.JSArrayImpl<T>(toJSValue!);
}

@patch
extension ListToJSArray<T extends JSAny?> on List<T> {
  JSArray<T>? get _underlyingArray {
    final t = this;
    return t is js_types.JSArrayImpl
        // Explicit cast to avoid using the extension method.
        ? JSArray<T>._((t as js_types.JSArrayImpl).toJSValue)
        : null;
  }

  @patch
  JSArray<T> get toJS => _underlyingArray ?? toJSArray<T>(this);

  @patch
  JSArray<T> get toJSProxyOrRef =>
      _underlyingArray ?? _createJSProxyOfList<T>(this);
}

// -----------------------------------------------------------------------------
// List.toJSProxyOrRef: a JavaScript Proxy over the Dart list, as in dart2wasm
// (`_ListBackedJSArray`, `_createJSProxyOfList`).

/// `createJSInteropWrapper` with the prototype `proto` (dart2wasm's
/// `createStaticInteropMock`): lowered like it (`lower/js_members.rs`).
external JSObject _createJSInteropWrapperOn<T extends Object>(
    T dartObject, JSObject proto);

@pragma('dartforge:js', '() => Array.prototype')
external JSValue _arrayPrototype();

// Implementation of indexing, `length`, and core handler methods.
//
// JavaScript's `Array` methods are similar to Dart's `ListMixin`, because they
// only rely on the implementation of `length` and indexing methods (and
// support for any JS operators like `in` or `delete`).
// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array#generic_array_methods
class _ListBackedJSArray {
  final List<JSAny?> _list;
  // The proxy that wraps this list.
  late final JSArray proxy;

  _ListBackedJSArray(this._list);

  @JSExport()
  int get length => _list.length;

  // Resizing the list populates the list with `null`, as in dart2wasm.
  @JSExport()
  void set length(int val) => _list.length = val;

  // []
  @JSExport()
  JSAny? _getIndex(int index) => _list[index];

  // []=
  @JSExport()
  void _setIndex(int index, JSAny? value) {
    // Need to resize the array if out of bounds.
    if (index >= length) length = index + 1;
    _list[index] = value;
  }

  // in
  @JSExport()
  bool _hasIndex(int index) => index >= 0 && index < length;

  // delete
  @JSExport()
  bool _deleteIndex(int index) {
    if (_hasIndex(index)) {
      _list.removeAt(index);
      return true;
    }
    return false;
  }
}

@pragma('dartforge:js', '''(wrapper) => {
  wrapper[Symbol.isConcatSpreadable] = true;
  const getIndex = wrapper._getIndex;
  const setIndex = wrapper._setIndex;
  const hasIndex = wrapper._hasIndex;
  const deleteIndex = wrapper._deleteIndex;
  return new Proxy(wrapper, {
    'get': function (target, prop, receiver) {
      if (typeof prop == 'string') {
        const numProp = Number(prop);
        if (Number.isInteger(numProp)) {
          return Reflect.apply(getIndex, wrapper, [numProp]);
        }
      }
      return Reflect.get(target, prop, receiver);
    },
    'set': function (target, prop, value, receiver) {
      if (typeof prop == 'string') {
        const numProp = Number(prop);
        if (Number.isInteger(numProp)) {
          Reflect.apply(setIndex, wrapper, [numProp, value]);
          return true;
        }
      }
      // Note that handler set is required to return a bool (whether it
      // succeeded or not), so `[]=` won't return the value set.
      return Reflect.set(target, prop, value, receiver);
    },
    'has': function (target, prop) {
      if (typeof prop == 'string') {
        const numProp = Number(prop);
        if (Number.isInteger(numProp)) {
          // Array-like objects are assumed to have indices as properties.
          return Reflect.apply(hasIndex, wrapper, [numProp]);
        }
      }
      return Reflect.has(target, prop);
    },
    'deleteProperty': function (target, prop) {
      if (typeof prop == 'string') {
        const numProp = Number(prop);
        if (Number.isInteger(numProp)) {
          return Reflect.apply(deleteIndex, wrapper, [numProp]);
        }
      }
      return Reflect.deleteProperty(target, prop);
    }
  });
}''')
external JSValue _listProxy(JSValue wrapper);

JSArray<T> _createJSProxyOfList<T extends JSAny?>(List<T> list) {
  final wrapper = _ListBackedJSArray(list);
  final jsExportWrapper = _createJSInteropWrapperOn<_ListBackedJSArray>(
      wrapper, JSObject._(_arrayPrototype()));
  final proxy = JSArray<T>._(_listProxy(jsExportWrapper.toJSValue));
  wrapper.proxy = proxy;
  return proxy;
}

// -----------------------------------------------------------------------------
// JSNumber -> double or int
@patch
extension JSNumberToNumber on JSNumber {
  @patch
  double get toDartDouble => toDartNumber(toJSValue);

  @patch
  int get toDartInt {
    final number = toDartNumber(toJSValue);
    final intVal = number.toInt();
    if (number == intVal) {
      return intVal;
    } else {
      throw 'Expected integer value, but was not integer.';
    }
  }
}

@patch
extension DoubleToJSNumber on double {
  @patch
  JSNumber get toJS => JSNumber._(toJSNumber(this));
}

// -----------------------------------------------------------------------------
// JSBoolean <-> bool
@patch
extension JSBooleanToBool on JSBoolean {
  @patch
  bool get toDart => toDartBool(toJSValue);
}

@patch
extension BoolToJSBoolean on bool {
  @patch
  JSBoolean get toJS => JSBoolean._(toJSBoolean(this));
}

// -----------------------------------------------------------------------------
// JSString <-> String
@patch
extension JSStringToString on JSString {
  @patch
  String get toDart => jsStringToDartString(toJSValue!);
}

@patch
extension StringToJSString on String {
  @patch
  JSString get toJS => JSString._(jsStringFromDartString(this));
}

@patch
extension JSAnyOperatorExtension on JSAny? {
  @patch
  JSAny add(JSAny? any) => JSAny._(_add(toJSValue, any.toJSValue));

  @patch
  JSAny subtract(JSAny? any) => JSAny._(_subtract(toJSValue, any.toJSValue));

  @patch
  JSAny multiply(JSAny? any) => JSAny._(_multiply(toJSValue, any.toJSValue));

  @patch
  JSAny divide(JSAny? any) => JSAny._(_divide(toJSValue, any.toJSValue));

  @patch
  JSAny modulo(JSAny? any) => JSAny._(_modulo(toJSValue, any.toJSValue));

  @patch
  JSAny exponentiate(JSAny? any) =>
      JSAny._(_exponentiate(toJSValue, any.toJSValue));

  @patch
  JSBoolean greaterThan(JSAny? any) =>
      JSBoolean._(_greaterThan(toJSValue, any.toJSValue));

  @patch
  JSBoolean greaterThanOrEqualTo(JSAny? any) =>
      JSBoolean._(_greaterThanOrEqualTo(toJSValue, any.toJSValue));

  @patch
  JSBoolean lessThan(JSAny? any) =>
      JSBoolean._(_lessThan(toJSValue, any.toJSValue));

  @patch
  JSBoolean lessThanOrEqualTo(JSAny? any) =>
      JSBoolean._(_lessThanOrEqualTo(toJSValue, any.toJSValue));

  @patch
  JSBoolean equals(JSAny? any) => JSBoolean._(_equals(toJSValue, any.toJSValue));

  @patch
  JSBoolean notEquals(JSAny? any) =>
      JSBoolean._(_notEquals(toJSValue, any.toJSValue));

  @patch
  JSBoolean strictEquals(JSAny? any) =>
      JSBoolean._(_strictEquals(toJSValue, any.toJSValue));

  @patch
  JSBoolean strictNotEquals(JSAny? any) =>
      JSBoolean._(_strictNotEquals(toJSValue, any.toJSValue));

  @patch
  JSNumber unsignedRightShift(JSAny? any) =>
      JSNumber._(_unsignedRightShift(toJSValue, any.toJSValue));

  @patch
  JSAny? and(JSAny? any) => _and(toJSValue, any.toJSValue) as JSAny?;

  @patch
  JSAny? or(JSAny? any) => _or(toJSValue, any.toJSValue) as JSAny?;

  @patch
  JSBoolean get not => JSBoolean._(_not(toJSValue));

  @patch
  JSBoolean get isTruthy => JSBoolean._(_isTruthy(toJSValue));
}

@pragma('dartforge:js', '(o, a) => o + a')
external JSValue _add(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o - a')
external JSValue _subtract(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o * a')
external JSValue _multiply(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o / a')
external JSValue _divide(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o % a')
external JSValue _modulo(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o ** a')
external JSValue _exponentiate(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o > a')
external JSValue _greaterThan(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o >= a')
external JSValue _greaterThanOrEqualTo(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o < a')
external JSValue _lessThan(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o <= a')
external JSValue _lessThanOrEqualTo(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o == a')
external JSValue _equals(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o != a')
external JSValue _notEquals(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o === a')
external JSValue _strictEquals(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o !== a')
external JSValue _strictNotEquals(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o >>> a')
external JSValue _unsignedRightShift(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o && a')
external JSValue? _and(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o, a) => o || a')
external JSValue? _or(JSValue? o, JSValue? a);

@pragma('dartforge:js', '(o) => !o')
external JSValue _not(JSValue? o);

@pragma('dartforge:js', '(o) => !!o')
external JSValue _isTruthy(JSValue? o);

@patch
JSPromise<JSObject> importModule(JSAny moduleName) =>
    JSPromise<JSObject>._(_importModule(moduleName.toJSValue));

@pragma('dartforge:js', '(m) => import(m)')
external JSValue _importModule(JSValue? m);
