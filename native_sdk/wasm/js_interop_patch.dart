// The `dart:js_interop` patch of DartForge's WebAssembly target
// (`dartforge_wasm`): dart2wasm's (`_internal/wasm/lib/js_interop_patch.dart`)
// over the handles of `dart:_js_helper`, with the same results and errors.

import 'dart:_internal' show patch;
import 'dart:_js_helper' as js_helper;
import 'dart:_js_helper' hide JS;
import 'dart:_js_types' as js_types;
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
