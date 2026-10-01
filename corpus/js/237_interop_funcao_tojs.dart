// diverge-ddc: `dart:js_interop` só existe na web; na VM o programa nem compila.
// `Function.toJS` do `dart:js_interop`: o transformador de interop troca o
// getter pelo `_functionToJS{N}` do `dart:js_util`, N = parâmetros
// posicionais do tipo estático. Implícito, pela aplicação explícita com
// prefixo e sobre um `as` — a forma que o template do ngdart escreve:
// `importN.FunctionToJSExportedDartFunction((h as void Function(E))).toJS`.
@JS()
library interop_funcao;

import 'dart:js_interop';
import 'dart:js_interop' as ji;

@JS('JSON.stringify')
external String stringify(JSAny? o, JSFunction substituto);

@JS('JSON.parse')
external JSAny? parse(String texto);

void main() {
  final chaves = <String>[];
  JSAny? f(JSString k, JSAny? v) {
    chaves.add(k.toDart);
    return v;
  }

  print(stringify(parse('{"a":1,"b":[2]}'), f.toJS));
  print(chaves);

  final g = ji.FunctionToJSExportedDartFunction((JSString k, JSAny? v) {
    chaves.add('g:${k.toDart}');
    return v;
  }).toJS;
  print(stringify(parse('{"c":3}'), g));

  Object h = (JSString k, JSAny? v) {
    chaves.add('h:${k.toDart}');
    return v;
  };
  final conv =
      ji.FunctionToJSExportedDartFunction((h as JSAny? Function(JSString, JSAny?))).toJS;
  print(stringify(parse('{"d":4}'), conv));
  print(chaves);
}
