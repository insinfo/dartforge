// Results of type T (a type parameter of the member or of its extension
// type): cast to the instantiation, so null fails a non-nullable T and any
// JS value passes; JSString.toDart of a value that is not a string.
import 'dart:js_interop';

@JS('eval')
external JSAny? jsEval(String code);

extension type Box._(JSObject _) implements JSObject {
  external Box(int v);
  external int get v;
  external T echo<T extends JSAny?>(JSAny? x);
  external T? maybe<T extends JSAny?>(JSAny? x);
}

extension type Holder<T extends JSAny?>._(JSObject _) implements JSObject {
  external Holder(T v);
  external T get v;
  external T call2(JSAny? x);
}

T first<T extends JSAny?>(JSArray<T> a) => a[0];

void main() {
  jsEval('globalThis.Box = class { constructor(v) { this.v = v; } echo(x) { return x; } maybe(x) { return x; } }');
  jsEval('globalThis.Holder = class { constructor(v) { this.v = v; } call2(x) { return x; } }');
  final b = Box(5);
  print(b.echo<JSString>('x'.toJS).toDart);
  print(b.echo<JSNumber>(3.toJS).toDartInt);
  print(b.maybe<JSString>(null));
  final h = Holder<JSString>('s'.toJS);
  print(h.v.toDart);
  print(first(<JSNumber>[9.toJS].toJS).toDartInt);
  try {
    final JSString bad = b.echo<JSString>(1.toJS);
    print(bad.toDart);
  } catch (e) {
    print('error 1 ${e.runtimeType}');
  }
  try {
    final JSNumber n = b.echo<JSNumber>(null);
    print(n);
  } catch (e) {
    print('error 2 ${e.runtimeType}');
  }
  try {
    final JSString s = h.call2(null);
    print(s);
  } catch (e) {
    print('error 3 ${e.runtimeType}');
  }
  final hn = Holder<JSString?>(null);
  print(hn.v);
  print(hn.call2(null));
}
