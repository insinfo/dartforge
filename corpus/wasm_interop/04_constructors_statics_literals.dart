// Interop constructors (`new ns1.Date(...)`, without `globalThis`), statics
// with and without `@JS` renames, object literals by the shape of the named
// arguments, a dotted `@JS('a.b.c')` getter, the library's `@JS('ns1')`
// prefix, extension members on JSObject and the int results of non-integral
// JavaScript values.
@JS('ns1')
library;

import 'dart:js_interop';
import 'dart:js_interop_unsafe';

@JS('Date')
extension type JSDate._(JSObject _) implements JSObject {
  external JSDate(int y, int m, int d);
  external static double now2();
  @JS('now')
  external static double nowRenamed();
  external int getFullYear();
}

extension type Opts._(JSObject _) implements JSObject {
  external factory Opts({int a, String? b, bool cc});
}

@JS()
extension type Obj._(JSObject _) implements JSObject {
  external int get i;
}

extension E on JSObject {
  external int get i;
  @JS('i')
  external int get renamedI;
}

@JS('a.b.c')
external int get abc;

@JS()
external int top(int x);

JSAny? jsEval(String code) =>
    globalContext.callMethod<JSAny?>('eval'.toJS, code.toJS);

String stringify(JSAny? o) =>
    (globalContext['JSON'] as JSObject).callMethod<JSString>('stringify'.toJS, o).toDart;

void t(String name, Object? Function() f) {
  try {
    final v = f();
    print('$name: $v ${v.runtimeType}');
  } catch (e) {
    print('$name: threw ${e.runtimeType}: $e');
  }
}

void main() {
  jsEval('globalThis.ns1 = {Date: Date, a: {b: {c: 42}}, top(x) { return x * 2; }}; Date.now2 = () => 1.5;');
  t('date', () => JSDate(2020, 1, 2).getFullYear());
  t('now', () => JSDate.nowRenamed() > 0);
  t('now2', () => JSDate.now2());
  t('abc', () => abc);
  t('top', () => top(21));
  print(stringify(Opts(a: 1, cc: true)));
  print(stringify(Opts(b: null)));
  print(stringify(Opts()));
  for (final v in ['"5"', 'undefined', 'true', '1e300', '-0', 'NaN', '2**53']) {
    jsEval('globalThis.z = {i: $v}');
    final z = jsEval('globalThis.z') as Obj;
    t('i=$v', () => z.i);
  }
  final w = jsEval('({i: 7})') as JSObject;
  t('ext', () => w.i);
  t('extren', () => w.renamedI);
}
