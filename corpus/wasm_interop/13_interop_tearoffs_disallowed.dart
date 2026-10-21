// Tear-offs of external interop members are compile errors, as in dart2wasm
// (_js_interop_checks): @staticInterop, extension, extension type and
// top-level members; non-external members and getters are not tear-offs.
@JS()
library t2;

import 'dart:js_interop';

@JS()
@staticInterop
class Old {
  external factory Old();
  external static int sv();
}

extension OldX on Old {
  external int m();
  external int get g;
}

extension ObjX on JSObject {
  external int n();
  int dartOne() => 1;
}

extension type E._(JSObject _) implements JSObject {
  external E.named();
  external factory E.fac();
  external int get g;
  external set s(int v);
  int notExternal() => 2;
}

@JS()
external int topVar;

@JS()
external int get topGetter;

void main() {
  final a = Old.new;
  final b = Old.sv;
  final o = Old();
  final c = o.m;
  final d = ObjX(o as JSObject).n;
  final e = (o as JSObject).n;
  final f = E.named;
  final g = E.fac;
  final h = (o as JSObject).dartOne;
  final i = (o as E).notExternal;
  print([a, b, c, d, e, f, g, h, i, topVar, topGetter]);
}
