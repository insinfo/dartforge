// diverge-ddc: `dart:js_interop` só existe na web; na VM o programa nem compila.
// `isA<T>()` do `dart:js_interop`: o transformador de interop troca a chamada
// pela checagem de tipo JS — `instanceof` pelo nome JS de um tipo de extensão
// de interop (o `isA<web.HTMLElement>()` do package:web), `typeof` para os
// primitivos, `T?` aceitando `null`.
@JS()
library interop_is_a;

import 'dart:js_interop';

@JS('JSON.parse')
external JSAny? parse(String texto);

@JS('Array')
extension type Lista._(JSObject _) implements JSObject {
  external int get length;
}

@JS('Date')
extension type Data._(JSObject _) implements JSObject {
  external Data(int ms);
}

void main() {
  final JSAny? arr = parse('[1,2,3]');
  final JSAny? obj = parse('{"a":1}');
  final JSAny? txt = parse('"oi"');
  final JSAny? nada = parse('null');
  print(arr.isA<Lista>());
  print(obj.isA<Lista>());
  print(txt.isA<JSString>());
  print(arr.isA<JSString>());
  print(nada.isA<Lista>());
  print(nada.isA<Lista?>());
  final JSAny d = Data(0);
  print(d.isA<Data>());
  print(d.isA<Lista>());
  if (arr.isA<Lista>()) {
    print((arr as Lista).length);
  }
}
