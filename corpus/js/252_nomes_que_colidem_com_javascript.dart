// Nomes Dart que colidem com o JavaScript gerado: construtores nomeados
// `length`, `caller` e `arguments` (propriedades somente leitura de funções
// JS) e um construtor com o nome de um getter estático da superclasse (os
// estáticos não são herdados: `Bar.baz()` é o construtor); uma classe com o
// nome de uma palavra reservada do JS; um local do corpo com o nome de um
// parâmetro `this.y`; dois inicializadores de campo com temporários; e um
// literal hexadecimal além de 64 bits num contexto `double`.
class T {
  final int l;
  const T.length(this.l);
  T.caller(this.l);
  T.arguments(this.l);
  T.name(this.l);
}

class Foo {
  static const baz = 1;
  static int get qux => 2;
}

class Bar extends Foo {
  final String how;
  Bar.baz() : how = 'baz';
  Bar.qux() : how = 'qux';
}

class debugger {
  String toString() => 'debugger';
}

class C {
  int? y, localY;
  C.initY(this.y) {
    var y = 123;
    localY = y;
  }
}

class Fields {
  List<int> a = [for (var i = 0; i < 3; i++) i];
  List<int> b = [for (var i = 0; i < 2; i++) i * 10];
}

void main() {
  print([const T.length(1).l, T.caller(2).l, T.arguments(3).l, T.name(4).l]);
  print([Bar.baz().how, Bar.qux().how, Foo.baz, Foo.qux]);
  print(debugger());
  final c = C.initY(7);
  print([c.y, c.localY]);
  final f = Fields();
  print([f.a, f.b]);
  double big = 0x10000000000000000;
  print(big == 18446744073709551616.0);
}
