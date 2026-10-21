// JSArray.toDart is a live List over the JavaScript array (dart2wasm's
// JSArrayImpl): reads, writes, growth, removal, sorting and views reach the
// array; List.toJS copies a Dart list and keeps a JSArrayImpl's own array.
import 'dart:js_interop';
import 'dart:js_interop_unsafe';

JSAny? jsEval(String code) =>
    globalContext.callMethod<JSAny?>('eval'.toJS, code.toJS);

String show(JSAny? a) =>
    (globalContext['JSON'] as JSObject).callMethod<JSString>('stringify'.toJS, a).toDart;

void main() {
  final js = jsEval('[3, 1, 2]') as JSArray<JSNumber>;
  final l = js.toDart;
  print(l.length);
  print(l.map((e) => e.toDartInt).toList());
  l.add(5.toJS);
  l.insert(0, 9.toJS);
  print(show(js));
  l.removeAt(1);
  l.removeLast();
  print(show(js));
  l.sort((a, b) => a.toDartInt.compareTo(b.toDartInt));
  print(show(js));
  l[0] = 7.toJS;
  print(show(js));
  print(l.sublist(1).map((e) => e.toDartDouble).toList());
  print(l.join('|'));
  l.length = 1;
  print(show(js));
  print(l.indexOf(l[0]));
  print(l.contains(7.toJS));
  print(l.reversed.map((e) => e.toDartInt).toList());
  final copy = <JSAny?>[1.toJS, 'a'.toJS, null, true.toJS].toJS;
  print(show(copy));
  print(identical(l.toJS, js) || (l.toJS as JSAny).strictEquals(js).toDart);
  try {
    l[5];
  } catch (e) {
    print(e.runtimeType);
  }
  print(l);
  final empty = JSArray<JSString>();
  empty[0] = 'x'.toJS;
  print('${empty.length} ${show(empty)}');
  print(JSArray<JSAny>.withLength(3).length);
}
