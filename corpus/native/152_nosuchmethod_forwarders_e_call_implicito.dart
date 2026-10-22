// Forwarders de `noSuchMethod` com o comportamento da VM: os argumentos e o
// valor do setter são verificados contra o membro da interface, o resultado é
// convertido ao tipo de retorno declarado (um `bool` onde se declara `String` é
// `TypeError`), o tear-off dinâmico de um método encaminhado é uma closure, e a
// chamada a um tipo de interface implementado só por um mock não é
// desvirtualizada para a implementação concreta. Também o tear-off implícito de
// `call` (`Function f = objeto;`) e a instanciação aos limites de uma chamada
// dinâmica genérica sem argumentos de tipo.
class Cat {
  bool eatFood(String food) => true;
  String scratch(String furniture) => 'purr';
  String mood = '';
}

class MockCat implements Cat {
  dynamic noSuchMethod(Invocation invocation) {
    var arg = invocation.positionalArguments.isEmpty ? null : invocation.positionalArguments[0];
    return arg is String && arg.isNotEmpty;
  }
}

class MockWithGenerics {
  List<Type> doStuff<T>(T t);
  noSuchMethod(i) => (i as dynamic).typeArguments;
}

class Callable {
  int call() => 1;
}

abstract class I {
  int call();
}

class Mock implements I {
  noSuchMethod(i) => 42;
}

class C {
  Type gen<T>(T x) => T;
  Type bounded<T extends num>(T x) => T;
}

void t(String n, void Function() f) {
  try {
    f();
    print('$n: no throw');
  } catch (e) {
    print('$n: ${e.runtimeType}');
  }
}

void main() {
  Cat c = MockCat();
  print(c.eatFood('y'));
  print(c.eatFood(''));
  MockCat mock = MockCat();
  t('dyn set', () {
    (mock as dynamic).mood = 42;
  });
  t('static set', () {
    mock.mood = 'x';
  });
  t('ret', () => mock.scratch("couch") + '');
  t('ret dyn', () => (mock as dynamic).scratch("couch"));
  t('arg', () => (mock as dynamic).eatFood(1));
  var eat = (mock as dynamic).eatFood;
  print(eat is bool Function(String));
  print(eat('z'));

  var g = MockWithGenerics();
  print(g.doStuff(42));
  print(g.doStuff<num>(42));
  dynamic d = g.doStuff;
  print(d<num>(42));
  print(d(42));

  var cl = Callable();
  Function f = cl;
  print(f == cl.call);
  print(identical(f, cl));
  print(f.runtimeType);
  var m = Mock();
  Function h = m;
  print(identical(h, m));
  print(h());

  dynamic o = C();
  print(o.gen(5));
  print(o.gen<int>(5));
  print(o.bounded(5));
}
