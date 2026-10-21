// JavaScript exceptions: caught in Dart as dart2wasm's _JavaScriptError, an
// uncaught one at the end, and 200 000 JS objects whose handles the collector
// returns to the adapter's table.
import 'dart:js_interop';
import 'dart:js_interop_unsafe';

void main() {
  try {
    globalContext.callMethod('eval'.toJS, 'throw new Error("boom")'.toJS);
  } catch (e) {
    print('caught ${e.runtimeType} $e');
  }
  try {
    globalContext.callMethod('eval'.toJS, 'throw 42'.toJS);
  } on Error catch (e) {
    print('caught error ${e.runtimeType}');
  }
  var total = 0;
  for (var i = 0; i < 200000; i++) {
    final o = JSObject();
    o['v'] = i.toJS;
    total += (o['v'] as JSNumber).toDartInt;
  }
  print(total);
  final s = StringBuffer();
  for (var i = 0; i < 3000; i++) {
    s.write(String.fromCharCode(i));
  }
  final big = s.toString();
  print((big.toJS).toDart == big);
  print(''.toJS.toDart.isEmpty);
  globalContext.callMethod('eval'.toJS, 'throw new TypeError("final")'.toJS);
}
