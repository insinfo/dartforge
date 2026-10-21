// The conversions of interop members by declared type (dart2wasm 3.6.2):
// int requires an integral number, double/num/bool/String go through dartify
// and a cast, a non-nullable JS type with undefined is a null check; only the
// arguments the call writes are passed.
import 'dart:js_interop';

@JS('eval')
external JSAny? jsEval(String code);

@JS()
extension type Obj._(JSObject _) implements JSObject {
  external int get i;
  external double get d;
  external num get n;
  external bool get b;
  external String get s;
  external int? get ni;
  external bool? get nb;
  external String? get ns;
  external JSObject get o;
  external JSObject? get no;
  external set i(int v);
  external set b(bool v);
  external set ns(String? v);
  external set nb(bool? v);
  external set ni(int? v);
  external int f(int a, [int? b, String? c]);
  external String g([JSAny? a, JSAny? b]);
}

void t(String name, Object? Function() f) {
  try {
    final v = f();
    print('$name: $v ${v.runtimeType}');
  } catch (e) {
    print('$name: threw ${e.runtimeType}: $e');
  }
}

void main() {
  jsEval('globalThis.x = {i: 3, d: 2.5, n: 7, b: true, s: "str", ni: null, nb: undefined, ns: "z", o: {}, no: null, f(a, b, c) { return arguments.length * 100 + a; }, g() { return "n=" + arguments.length + ":" + typeof arguments[0]; }}');
  final x = jsEval('globalThis.x') as Obj;
  t('i', () => x.i);
  t('d', () => x.d);
  t('n', () => x.n);
  t('b', () => x.b);
  t('s', () => x.s);
  t('ni', () => x.ni);
  t('nb', () => x.nb);
  t('ns', () => x.ns);
  t('o', () => x.o);
  t('no', () => x.no);
  t('f1', () => x.f(1));
  t('f2', () => x.f(1, 2));
  t('f3', () => x.f(1, null, 'c'));
  t('g0', () => x.g());
  t('g1', () => x.g(null));
  t('g2', () => x.g(1.toJS, null));
  x.i = 5;
  x.b = false;
  x.ns = null;
  x.nb = true;
  x.ni = 9;
  print(jsEval('JSON.stringify([x.i, x.b, x.ns, x.nb, x.ni])'));
  jsEval('globalThis.y = {i: 2.5, b: 1, s: 5, o: undefined, d: "q"}');
  final y = jsEval('globalThis.y') as Obj;
  t('yi', () => y.i);
  t('yb', () => y.b);
  t('ys', () => y.s);
  t('yo', () => y.o);
  t('yd', () => y.d);
  t('yn', () => y.n);
}
