// isA<T>(): typeof for the primitive JS types, instanceof by JS name for the
// others (the typed arrays' common prototype for JSTypedArray), T? accepts null.
import 'dart:js_interop';

@JS('eval')
external JSAny? jsEval(String code);

@JS('Map')
extension type JSMap._(JSObject _) implements JSObject {}
extension type Plain._(JSObject _) implements JSObject {}

void main() {
  final values = <String, JSAny?>{
    'num': 1.toJS,
    'str': 'a'.toJS,
    'bool': true.toJS,
    'obj': jsEval('({})'),
    'arr': jsEval('[1]'),
    'fn': jsEval('(() => 1)'),
    'map': jsEval('new Map()'),
    'u8': jsEval('new Uint8Array(2)'),
    'prom': jsEval('Promise.resolve(1)'),
    'null': null,
    'sym': jsEval('Symbol("s")'),
    'big': jsEval('10n'),
    'box': Object().toJSBox,
  };
  for (final e in values.entries) {
    final v = e.value;
    print('${e.key}: any=${v.isA<JSAny>()} any?=${v.isA<JSAny?>()} obj=${v.isA<JSObject>()} num=${v.isA<JSNumber>()} '
        'str=${v.isA<JSString>()} bool=${v.isA<JSBoolean>()} arr=${v.isA<JSArray>()} fn=${v.isA<JSFunction>()} '
        'prom=${v.isA<JSPromise>()} u8=${v.isA<JSUint8Array>()} map=${v.isA<JSMap>()} plain=${v.isA<Plain>()} '
        'sym=${v.isA<JSSymbol>()} big=${v.isA<JSBigInt>()} str?=${v.isA<JSString?>()} box=${v.isA<JSBoxedDartObject>()}');
  }
}
