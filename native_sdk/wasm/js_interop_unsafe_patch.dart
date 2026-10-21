// The `dart:js_interop_unsafe` patch of DartForge's WebAssembly target
// (`dartforge_wasm`): dart2wasm's (`_internal/wasm/lib/
// js_interop_unsafe_patch.dart`) over the handles of `dart:_js_helper`.

import 'dart:_internal' show patch;
import "dart:_js_helper";
import 'dart:js_interop' hide JS;

T _box<T>(JSValue? ref) => ref as T;

@patch
extension JSObjectUnsafeUtilExtension on JSObject {
  @patch
  JSBoolean hasProperty(JSAny property) =>
      _box<JSBoolean>(_hasProperty(toJSValue, property.toJSValue));

  @patch
  T getProperty<T extends JSAny?>(JSAny property) =>
      _box<T>(getPropertyRaw(toJSValue, property.toJSValue));

  @patch
  void setProperty(JSAny property, JSAny? value) =>
      _setProperty(toJSValue, property.toJSValue, value.toJSValue);

  @patch
  JSAny? _callMethod(JSAny method,
          [JSAny? arg1, JSAny? arg2, JSAny? arg3, JSAny? arg4]) =>
      _box<JSAny?>(callMethodVarArgsRaw(
          toJSValue,
          method.toJSValue,
          [
            if (arg1 != null) arg1,
            if (arg2 != null) arg2,
            if (arg3 != null) arg3,
            if (arg4 != null) arg4,
          ].toJSValue));

  @patch
  JSAny? _callMethodVarArgs(JSAny method, [List<JSAny?>? arguments]) =>
      _box<JSAny?>(callMethodVarArgsRaw(toJSValue, method.toJSValue,
          (arguments ?? <JSAny?>[]).toJSValue));

  @patch
  JSBoolean delete(JSAny property) =>
      _box<JSBoolean>(_delete(toJSValue, property.toJSValue));
}

@patch
extension JSFunctionUnsafeUtilExtension on JSFunction {
  @patch
  JSObject _callAsConstructor(
          [JSAny? arg1, JSAny? arg2, JSAny? arg3, JSAny? arg4]) =>
      _box<JSObject>(callConstructorVarArgsRaw(
          toJSValue,
          [
            if (arg1 != null) arg1,
            if (arg2 != null) arg2,
            if (arg3 != null) arg3,
            if (arg4 != null) arg4,
          ].toJSValue));

  @patch
  JSObject _callAsConstructorVarArgs([List<JSAny?>? arguments]) =>
      _box<JSObject>(callConstructorVarArgsRaw(
          toJSValue, (arguments ?? <JSAny?>[]).toJSValue));
}

@pragma('dartforge:js', '(o, p) => p in o')
external JSValue? _hasProperty(JSValue? o, JSValue? p);

@pragma('dartforge:js', '(o, p, v) => o[p] = v')
external void _setProperty(JSValue? o, JSValue? p, JSValue? v);

@pragma('dartforge:js', '(o, p) => delete o[p]')
external JSValue? _delete(JSValue? o, JSValue? p);
