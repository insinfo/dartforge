// Callbacks: Function.toJS (arity by arguments.length, extra arguments
// ignored, too few arguments an error), toJSCaptureThis, Dart exceptions
// through JavaScript in both directions, JSPromise.toDart (resolve, reject,
// reject with null), toJSBox, ExternalDartReference and a timer callback.
import 'dart:js_interop';
import 'dart:js_interop_unsafe';

@JS('Promise.resolve')
external JSPromise<JSNumber> resolved(JSNumber n);
@JS('Promise.reject')
external JSPromise<JSAny?> rejected(JSAny? n);
@JS('setTimeout')
external void setTimeout(JSFunction f, int ms);
@JS('eval')
external JSAny? jsEval(String code);

void main() async {
  final v = await resolved(5.toJS).toDart;
  print(v.toDartInt);
  try {
    await rejected('bad'.toJS).toDart;
  } catch (e) {
    print('rejected: ${e.runtimeType} $e');
  }
  try {
    await rejected(null).toDart;
  } catch (e) {
    print('rejected null: ${e.runtimeType} $e');
  }
  final add = ((int a, int b) => a + b).toJS;
  final call = jsEval('(f) => [f(1, 2), f(3, 4, 5), typeof f, f.length]') as JSFunction;
  print((call.callAsFunction(null, add) as JSArray).toDart.map((e) => e.dartify()).toList());
  final o = Object();
  final thrower = (() { if (o.hashCode != -1) throw StateError("from dart"); }).toJS;
  final callAndCatch = jsEval('(f) => { try { f(); return "no"; } catch (e) { return "js caught " + typeof e; } }') as JSFunction;
  print(callAndCatch.callAsFunction(null, thrower));
  final passThrough = jsEval('(f) => f()') as JSFunction;
  try {
    passThrough.callAsFunction(null, thrower);
  } catch (e) {
    print('dart caught ${e.runtimeType} $e');
  }
  final fewer = ((int? a, String? b) => '$a $b').toJS;
  try {
    print((jsEval('(f) => f(1)') as JSFunction).callAsFunction(null, fewer));
  } catch (e) {
    print('fewer: ${e.runtimeType} $e');
  }
  print((jsEval('(f) => f(1, undefined)') as JSFunction).callAsFunction(null, fewer));
  final opt = (([int? a, int? b]) => '$a $b').toJS;
  print((jsEval('(f) => [f(), f(1), f(1,2,3), f.length]') as JSFunction).callAsFunction(null, opt).dartify());
  final wrongType = ((int a) => a).toJS;
  try {
    print((jsEval('(f) => f("x")') as JSFunction).callAsFunction(null, wrongType));
  } catch (e) {
    print('wrongType: ${e.runtimeType} $e');
  }
  final retStr = (() => 'ret').toJS;
  print((jsEval('(f) => typeof f()') as JSFunction).callAsFunction(null, retStr));
  final retVoid = (() {}).toJS;
  print((jsEval('(f) => typeof f()') as JSFunction).callAsFunction(null, retVoid));
  final capture = ((JSAny? self, int a) => '${(self as JSObject)['k']} $a').toJSCaptureThis;
  print((jsEval('(f) => ({k: "kk", f}).f(3)') as JSFunction).callAsFunction(null, capture));
  final box = o.toJSBox;
  print(identical(box.toDart, o));
  print(box.typeofEquals('object'));
  final ref = o.toExternalReference;
  print(identical(ref.toDartObject, o));
  final f = (() => 7).toJS;
  print(identical(f.toDart, f.toDart));
  setTimeout((() { print('timeout'); }).toJS, 10);
  print('end main');
}
