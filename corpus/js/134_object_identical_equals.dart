// identical em const vs new, == por identidade e sobrescrito, Object.hash consistente, toString padrão e runtimeType de classes próprias.
class Simples {
  final int v;
  Simples(this.v);
  const Simples.c(this.v);
}

class ComIgual {
  final int a;
  final String b;
  const ComIgual(this.a, this.b);
  @override
  bool operator ==(Object other) => other is ComIgual && other.a == a && other.b == b;
  @override
  int get hashCode => Object.hash(a, b);
  @override
  String toString() => 'ComIgual($a, $b)';
}

class Sub extends ComIgual {
  const Sub(super.a, super.b);
}

class Generica<T> {
  final T valor;
  Generica(this.valor);
}

class SoHash {
  @override
  int get hashCode => 1;
}

void main() {
  var s1 = Simples(1);
  var s2 = Simples(1);
  print(identical(s1, s2));
  print(identical(s1, s1));
  print(s1 == s2);
  print(s1 == s1);
  print(s1 != s2);
  const c1 = Simples.c(1);
  const c2 = Simples.c(1);
  print(identical(c1, c2));
  print(c1 == c2);
  print(identical(const Simples.c(2), const Simples.c(2)));
  print(identical(Simples.c(2), Simples.c(2)));
  print(identical(const [1, 2], const [1, 2]));
  print(identical([1, 2], [1, 2]));
  print(identical(const {'a': 1}, const {'a': 1}));
  print(identical(const <int>{}, const <int>{}));
  print(identical('abc', 'abc'));
  print(identical(1, 1));
  print(identical(true, true));
  print(identical(null, null));
  print(identical(Object, Object));
  print(identical(Simples, Simples));
  print(identical(s1.v, s2.v));
  print(identical(const ComIgual(1, 'x'), const ComIgual(1, 'x')));
  print(identical(ComIgual(1, 'x'), ComIgual(1, 'x')));

  var i1 = ComIgual(1, 'x');
  var i2 = ComIgual(1, 'x');
  var i3 = ComIgual(2, 'x');
  print(i1 == i2);
  print(i1 == i3);
  print(i1 != i3);
  print(identical(i1, i2));
  print(i1.hashCode == i2.hashCode);
  print(i1 == Sub(1, 'x'));
  print(Sub(1, 'x') == i1);
  print(Sub(1, 'x') == Sub(1, 'x'));
  print(i1 == Object());
  print(Object() == i1);
  print(i1 == 'texto');
  print([i1].contains(i2));
  print([s1].contains(s2));
  print({i1, i2}.length);
  print({s1, s2}.length);
  print({i1: 'a'}[i2]);
  print({s1: 'a'}[s2]);
  print({i1: 'a'}[i1]);
  print([i1, i3].indexOf(ComIgual(2, 'x')));
  print(<ComIgual>{i1, i3}.contains(ComIgual(2, 'x')));
  Object o = i1;
  print(o == i2);
  print(o.hashCode == i2.hashCode);

  print(Object.hash(1, 2) == Object.hash(1, 2));
  print(Object.hash(1, 2) == Object.hash(2, 1));
  print(Object.hash('a', 'b') == Object.hash('a', 'b'));
  print(Object.hashAll([1, 2, 3]) == Object.hashAll([1, 2, 3]));
  print(Object.hashAll([1, 2, 3]) == Object.hashAll([3, 2, 1]));
  print(Object.hashAllUnordered([1, 2, 3]) == Object.hashAllUnordered([3, 2, 1]));
  print(Object.hash(1, 2, 3) == Object.hash(1, 2, 3));
  print(Object.hash(null, null) == Object.hash(null, null));
  print(1.hashCode == 1.hashCode);
  print('a'.hashCode == 'a'.hashCode);
  print(s1.hashCode == s1.hashCode);
  print(identityHashCode(s1) == identityHashCode(s1));
  print(identityHashCode(s1) == s1.hashCode);
  print(SoHash().hashCode);
  print(SoHash() == SoHash());
  print({SoHash(), SoHash()}.length);

  print(s1.toString());
  print(Simples(3));
  print('${Simples(3)}');
  print(Object().toString());
  print(Generica<int>(1).toString());
  print(Generica<String>('x').toString());
  print(Generica(2.5).toString());
  print(i1.toString());
  print(Sub(1, 'x').toString());
  print(ComIgual(1, 'x').toString() == 'ComIgual(1, x)');
  print(s1.runtimeType);
  print(s1.runtimeType == Simples);
  print(s1.runtimeType.toString());
  print(i1.runtimeType);
  print(Sub(1, 'x').runtimeType);
  print(Sub(1, 'x').runtimeType == ComIgual);
  print(Sub(1, 'x') is ComIgual);
  print(Generica<int>(1).runtimeType);
  print(Generica<String>('x').runtimeType);
  print(Generica<int>(1).runtimeType == Generica<int>(2).runtimeType);
  print(Generica<int>(1).runtimeType == Generica<String>('a').runtimeType);
  print(Generica<List<int>>([]).runtimeType);
  print(Generica<int?>(null).runtimeType);
  print(Generica<Simples>(s1).runtimeType);
  print(Object().runtimeType);
  print(Simples);
  print(ComIgual);
  print(Generica);
  print(o.runtimeType);
  print(o.runtimeType == ComIgual);
  print(o is ComIgual);
  print(o is Simples);
  print(o is Object);
  print(o is! Simples);
  print((o as ComIgual).a);
  Object? nulo;
  print(nulo == null);
  print(nulo.hashCode == null.hashCode);
  print(identical(nulo, null));
  print(nulo.runtimeType);
  print(nulo.toString());
  print([Simples(1), Simples(2)].map((e) => e.v).toList());
  var lista = [s1, s2, s1];
  print(lista.where((e) => identical(e, s1)).length);
  print(lista.where((e) => e == s1).length);
  print(lista.toSet().length);
  var eqs = [i1, i2, i3, ComIgual(1, 'x')];
  print(eqs.toSet().length);
  print(eqs.map((e) => e == i1).toList());
  print(eqs.map((e) => identical(e, i1)).toList());
}
