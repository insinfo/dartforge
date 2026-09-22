// diverge-ddc: `dart:js_interop` só existe na web; na VM o programa nem compila.
// Interop moderna (`dart:js_interop`): tipos de extensão sobre `JSObject`,
// construtor `external` (objeto JS) e construtor só com nomeados (literal de
// objeto), membros `external`, funções de topo `@JS`, e as conversões
// `toJS`/`toDart`/`jsify`/`dartify`.
@JS()
library interop;

import 'dart:js_interop';

@JS('JSON.stringify')
external String stringify(JSAny? o);

@JS('JSON.parse')
external JSAny? parse(String texto);

@JS('Object.keys')
external JSArray<JSString> chaves(JSObject o);

@JS('Math.max')
external int maximo(int a, int b);

extension type Caixa._(JSObject _) implements JSObject {
  external factory Caixa({int a, String b});
  external int get a;
  external set a(int v);
  external String get b;
}

extension type Contador._(JSObject _) implements JSObject {
  external factory Contador.novo();
  external int valor;
  external int incrementa(int n);
}

@JS('Array')
extension type JsLista._(JSObject _) implements JSObject {
  external factory JsLista();
  external int get length;
  external int push(JSAny? v);
  external String join(String sep);
}

void main() {
  final c = Caixa(a: 1, b: 'x');
  print(c.a);
  c.a = 5;
  print(c.a);
  print(c.b);
  print(stringify(c));
  print(chaves(c).toDart.map((s) => s.toDart).toList());

  print(maximo(3, 7));
  final analisado = parse('{"k":[1,2]}');
  print(stringify(analisado));

  final lista = JsLista();
  lista.push(10.toJS);
  lista.push('vinte'.toJS);
  print(lista.length);
  print(lista.join('-'));

  // Conversões.
  final s = 'texto'.toJS;
  print(s.toDart);
  final n = 3.5.toJS;
  print(n.toDartDouble);
  final b = true.toJS;
  print(b.toDart);
  final mapa = {'a': 1, 'b': <int>[2, 3]}.jsify();
  print(stringify(mapa));
  print((mapa as JSObject).dartify());

  // Tipos de extensão em posição de tipo (apagados para JSObject).
  JSAny? qualquer = c;
  print(qualquer is JSObject);
  final outra = qualquer as Caixa;
  print(outra.a);
  print(<Caixa>[c, Caixa(a: 2, b: 'y')].map((x) => x.a).toList());
}
