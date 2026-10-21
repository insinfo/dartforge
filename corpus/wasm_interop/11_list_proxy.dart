// List.toJSProxyOrRef: a JSArray that is a list from JavaScript (the same
// array), and a JavaScript Proxy over a Dart list (indexing, length, `in`,
// `delete`, Array.prototype methods, isConcatSpreadable), writes from
// JavaScript visible in Dart. (Growing the list from JavaScript fills it with
// null, which dart2wasm 3.6.2 rejects with a cast error; not covered.)
import 'dart:js_interop';

@JS('eval')
external JSAny? jsEval(String code);

JSAny? call(String code, JSAny? a, [JSAny? b]) =>
    (jsEval(code) as JSFunction).callAsFunction(null, a, b);

void main() {
  final dart = <JSNumber>[1.toJS, 2.toJS, 3.toJS];
  final proxy = dart.toJSProxyOrRef;
  print(call('(a) => a.length', proxy));
  print(call('(a) => a[1]', proxy));
  print(call('(a) => a.map((x) => x * 10).join(",")', proxy));
  print(call('(a) => Array.isArray(a)', proxy));
  print(call('(a) => a instanceof Array', proxy));
  print(call('(a) => 2 in a', proxy));
  print(call('(a) => 5 in a', proxy));
  call('(a) => { a[0] = 42; }', proxy);
  print(dart.map((n) => n.toDartInt).toList());
  dart.addAll([7.toJS, 8.toJS]);
  print(call('(a) => a.length', proxy));
  call('(a) => { delete a[0]; }', proxy);
  print(dart.length);
  print(call('(a) => [0].concat(a).length', proxy));
  print(call('(a) => a.indexOf(7)', proxy));
  call('(a) => { a.length = 2; }', proxy);
  print(dart.length);
  print(call('(a) => JSON.stringify(a.slice())', proxy));

  // A list from JavaScript is the same array.
  final fromJs = (jsEval('[4, 5]') as JSArray<JSNumber>).toDart;
  final back = fromJs.toJSProxyOrRef;
  print(call('(a) => { a.push(6); return a.length; }', back));
  print(fromJs.length);
  print(fromJs.last.toDartInt);
  print(call('(a, b) => a === b', back, fromJs.toJS));
}
