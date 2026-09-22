// diverge-ddc: interop JS (package:js) só existe na web; a VM não executa membros external.
// Classes `@JS`/`@anonymous`, membros `external` de instância e estáticos,
// funções e getters de topo, `allowInterop`, `dart:js_util`, tearoffs e
// testes de tipo (LegacyJavaScriptObject).
@JS()
library main;

import 'dart:js_util' as js_util;
import 'package:js/js.dart';

@JS('JSON.stringify')
external String stringify(dynamic o);

@JS('JSON.parse')
external dynamic parse(String s);

@JS('Math')
external dynamic get mathObj;

@JS()
@anonymous
class Opcoes {
  external factory Opcoes({int a, String? b, Function? cb});
  external int get a;
  external set a(int v);
  external String? get b;
  external Function? get cb;
  external set cb(Function? f);
  static int ajuda() => 7;
  factory Opcoes.padrao() => Opcoes(a: 42);
}

@JS('Math')
class Mat {
  external static num max(num a, num b);
  @JS('min')
  external static num minimo(num a, num b);
  external static num get PI;
  static num dobroPi() => PI * 2;
}

@JS('Array')
class JsArray {
  external factory JsArray();
  external int get length;
  external set length(int v);
  external int push(dynamic v);
  external void forEach(Function f);
  external JsArray map(Function f);
  external String join(String sep);
}

@JS('Object')
class JsObject {
  external static JsArray keys(dynamic o);
}

void main() {
  final o = Opcoes(a: 1, b: 'x');
  print(o.a);
  o.a = 5;
  print(o.a);
  print(o.b);
  print(stringify(o));
  print(Opcoes.ajuda());
  print(Opcoes.padrao().a);
  print(Mat.max(1, 2));
  print(Mat.minimo(1, 2));
  print(Mat.PI > 3);
  print(Mat.dobroPi() > 6);
  print(stringify(parse('{"k":[1,2]}')));
  print(mathObj != null);

  // Callbacks: allowInterop direto e via assertInterop.
  final arr = JsArray();
  arr.push(10);
  arr.push(20);
  print(arr.length);
  arr.forEach(allowInterop((v, i, a) => print('$i:$v')));
  final dobrado = arr.map(allowInterop((v, i, a) => v * 2));
  print(dobrado.join(','));
  o.cb = allowInterop(() => print('cb'));
  (o.cb as Function)();
  try {
    o.cb = () => print('sem allowInterop');
    print('atribuiu');
  } catch (e) {
    print('erro: assertInterop');
  }

  // js_util.
  print(js_util.getProperty(o, 'a'));
  js_util.setProperty(o, 'c', 3);
  print(js_util.hasProperty(o, 'c'));
  print(js_util.callMethod(arr, 'join', ['-']));
  print(stringify(js_util.jsify({'m': [1, 2]})));
  print(JsObject.keys(o).join('|'));

  // Tipos de interop nas verificações de tipo.
  Object x = o;
  print(x is Opcoes);
  print(x is JsArray);
  print((x as Opcoes).a);
  final lista = <Opcoes>[o, Opcoes(a: 2)];
  print(lista.map((e) => e.a).toList());
  Opcoes? nulo;
  print(nulo?.a);

  // Tearoffs.
  final max = Mat.max;
  print(max(3, 4));
  final fabrica = Opcoes.new;
  print(fabrica(a: 9).a);
  print(arr.length);
}
