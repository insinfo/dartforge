// Forwarding stubs de covariância: quando a implementação herdada (da
// superclasse ou de um mixin) não verifica um parâmetro que a interface da
// classe torna covariante, a classe verifica o argumento contra o tipo da
// implementação e chama `super`; o tear-off do stub tem o parâmetro como
// `Object?`. A instanciação implícita de uma função global ou estática com
// argumentos de tipo constantes é uma constante. Um `call` de extensão em
// `Object?` torna um valor anulável chamável.
class A {}

class B extends A {}

class C {
  void f(B x) {}
  void g(num x, [int y = 0, int z = 1]) => print('g $x $y $z');
}

abstract class I {
  void f(covariant A x);
  void g(covariant Object x, [covariant Object y, covariant Object z]);
}

class D extends C implements I {}

abstract class J<T> {
  void add(T t);
}

mixin class Base {
  void add(int x) => print('add $x');
}

class ViaMixin extends Object with Base implements J<int> {}

class ViaAlias = Object with Base implements J<int>;

class ViaSuper extends Base implements J<int> {}

void attempt(String name, void Function() f) {
  try {
    f();
  } catch (e) {
    print('$name threw');
  }
}

X? generic<X>() => null;

class S {
  static X? method<X>() => null;
}

const int? Function() constant = generic;
int? Function() variable = generic;

extension Callable on Object? {
  String call(Object a) => '$this($a)';
}

void main() {
  I i = D();
  i.f(B());
  attempt('f', () => i.f(A()));
  i.g(1, 2, 3);
  attempt('g', () => i.g('x', 2, 3));
  attempt('g y', () => i.g(1, 'y', 3));
  final tearOff = D().f;
  print(tearOff is void Function(Object?));
  attempt('tear-off', () => (tearOff as dynamic)(A()));

  for (final J<Object> j in <J<Object>>[ViaMixin(), ViaAlias(), ViaSuper()]) {
    j.add(1);
    attempt('${j.runtimeType}', () => j.add('s'));
  }

  int? Function() local = generic;
  int? Function() staticLocal = S.method;
  print(identical(constant, variable));
  print(identical(constant, local));
  print(identical(staticLocal, S.method<int>));

  Object? maybe = null;
  print(maybe('x'));
}
