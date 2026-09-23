// diverge-ddc: interop JS (package:js) só existe na web; a VM não executa membros external.
// Poda do perfil de produção (docs/JS-PRODUCAO.md §1.7) com interop: o
// mundo fechado não enxerga o que o JS chama, então o que atravessa a
// fronteira tem de sobreviver pela regra do contrato — callbacks Dart criados
// no corpo de quem os passa (`allowInterop`), fábricas e estáticos não
// externos de classes `@JS`, tearoff de fábrica `@anonymous` e classes Dart
// cujos métodos o JS chama de volta pelo callback. Se a poda errar, o caso
// imprime "caso <nome>: ERRO".
@JS()
library main;

import 'package:js/js.dart';

void caso(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('caso $nome: ERRO $e');
  }
}

@JS('JSON.stringify')
external String stringify(dynamic o);

@JS()
@anonymous
class Opcoes {
  external factory Opcoes({int a, Function? cb});
  external int get a;
  external Function? get cb;
  static int padraoA() => 7;
  factory Opcoes.padrao() => Opcoes(a: padraoA());
}

@JS('Array')
class JsArray {
  external factory JsArray();
  external int push(dynamic v);
  external void forEach(Function f);
  external JsArray map(Function f);
  external String join(String sep);
}

// Classe Dart chamada de volta pelo JS através de um callback.
class Acumulador {
  final List<int> itens = [];
  void guarda(int v) => itens.add(v);
  int total() => itens.fold(0, (a, b) => a + b);
  void nuncaChamado() => print('não devia imprimir');
}

int dobra(dynamic v) => (v as int) * 2;

void main() {
  caso('fábrica @anonymous', () => stringify(Opcoes(a: 1)));
  caso('fábrica não externa e estático', () => Opcoes.padrao().a);
  caso('tearoff de fábrica', () {
    final f = Opcoes.new;
    return f(a: 9).a;
  });
  caso('callback com classe Dart', () {
    final arr = JsArray();
    arr.push(1);
    arr.push(2);
    final acc = Acumulador();
    arr.forEach(allowInterop((v, i, a) => acc.guarda(v as int)));
    return acc.total();
  });
  caso('tearoff de topo como callback', () {
    final arr = JsArray();
    arr.push(3);
    arr.push(4);
    return arr.map(allowInterop((v, i, a) => dobra(v))).join(',');
  });
  caso('callback guardado em objeto JS', () {
    final o = Opcoes(a: 0, cb: allowInterop(() => 'chamado pelo JS'));
    return (o.cb as Function)();
  });
}
