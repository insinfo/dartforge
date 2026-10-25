// A ordem da VM ao chamar o valor de um getter: com receptor explícito, os
// argumentos antes do getter de instância ('AG'); com `this` implícito,
// `super` (também num mixin), getter estático, de extensão ou global, o getter
// antes ('GA'). Uma chamada a uma fábrica redirecionada usa os valores padrão
// do construtor-alvo (`{int z}` sem padrão passa `null` ao alvo). Os
// argumentos de tipo de uma chamada genérica são os que a inferência
// instanciou (o contexto `List<Object>` e os limites de uma função local
// chamada sem argumentos de tipo), e um tipo topo casado com um parâmetro de
// extensão de limite `Object` vira o limite.
String effects = '';
get arg {
  effects += 'A';
}

class A {
  void Function(void) get m {
    effects += 'G';
    return (_) {};
  }

  static void Function(void) get n {
    effects += 'G';
    return (_) {};
  }
}

class B extends A {
  void run() {
    order('explicit this', () => this.m(arg));
    order('implicit this', () => m(arg));
    order('super', () => super.m(arg));
  }
}

mixin M on A {
  void runMixin() => order('super in a mixin', () => super.m(arg));
}

class AM = A with M;

extension E on int {
  void Function(void) get m {
    effects += 'G';
    return (_) {};
  }
}

void order(String name, void Function() f) {
  effects = '';
  f();
  print('$name: $effects');
}

class P {
  int x, y, z;
  P({this.x = 3, this.y = 5, z}) : z = z ?? 2;
  factory P.redirect({int x, int y, int z}) = P;
  int get value => x * y * z;
}

List<T> one<T>(T x) => [x];

extension NotNull<T extends Object> on List<T?> {
  List<T> nonNull() => [for (final e in this) if (e != null) e];
}

void main() {
  final a = A();
  dynamic d = a;
  order('instance getter', () => a.m(arg));
  order('dynamic', () => d.m(arg));
  order('static getter', () => A.n(arg));
  order('extension getter', () => 1.m(arg));
  B().run();
  AM().runMixin();

  print([P.redirect().value, P.redirect(y: 7).value, P.redirect(z: 1).value]);
  final tearOff = P.redirect;
  print(tearOff(x: 2).value);

  List<Object> objects = one(1);
  objects.add('not an int');
  print(objects);

  T Function<T extends num>() local = <T extends num>() => (0 as T);
  print(local.runtimeType);
  void bar<X extends num, Y extends X>(List<X>? xs) => print('$X $Y');
  bar(null);

  const dynamic o = 42;
  final l = [o].nonNull();
  l.add(Object());
  print(l.length);
}
