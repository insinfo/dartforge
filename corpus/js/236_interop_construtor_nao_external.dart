// diverge-ddc: `dart:js_interop` só existe na web; na VM o programa nem compila.
// Construtor gerativo NÃO-external de tipo de extensão de interop — a forma
// do `HTMLStyleElement() : _ = document.createElement('style')` do
// package:web —, com membros `external` e não-`external`. Só o membro
// `external` é interop (`usesJSInterop`, dev_compiler js_interop.dart); o
// resto é Dart comum, como num tipo de extensão apagado.
@JS()
library interop_construtor;

import 'dart:js_interop';

@JS('JSON.stringify')
external String stringify(JSAny? o);

@JS('Object')
external JSObject novoObjeto();

extension type Ponto._(JSObject _) implements JSObject {
  // Gerativo com lista de inicializadores sobre a representação.
  Ponto() : _ = novoObjeto();

  // Gerativo com parâmetros, inicializador e corpo.
  Ponto.em(int x, int y) : _ = novoObjeto() {
    this.x = x;
    this.y = y;
  }

  external int x;
  external int y;
}

extension type Rotulo._(JSObject _) implements JSObject {
  Rotulo(String texto) : _ = novoObjeto() {
    valor = texto;
  }

  external String valor;
}

void main() {
  final p = Ponto();
  p.x = 3;
  p.y = 4;
  print(p.x + p.y);
  print(stringify(p));

  final q = Ponto.em(10, 20);
  print(q.x * q.y);
  print(stringify(q));

  final r = Rotulo('olá');
  print(r.valor);
  print(stringify(r));

  // Tear-off do construtor não-external.
  final fabrica = Rotulo.new;
  print(fabrica('tear-off').valor);
}
