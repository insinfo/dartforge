// Class type variables named by position in production recipes: the rti
// derives each one from the class's own arguments or from the supertype
// entry of the instance's class. Every evaluation must give the VM's type.

class Base<A, B> {
  List<A> firsts() => <A>[];
  Map<A, B> pairs() => <A, B>{};
  bool isFirst(Object? o) => o is A;
  bool isSecond(Object? o) => o is B;
  B castSecond(Object? o) => o as B;
  String describe() => '${A} ${B} ${<B, A>{} is Map<String, int>}';
}

// Reversed order: Base.A is Swap.Y, Base.B is Swap.X.
class Swap<X, Y> extends Base<Y, X> {
  List<X> xs() => <X>[];
  String own() => '${X} ${Y}';
}

class Fixed extends Swap<int, String> {}

class Lone<T> {
  // No supertype other than Object: no rule entry at all.
  List<T> make() => <T>[];
  bool test(Object? o) => o is T;
  T Function(T) identity() => (T t) => t;
}

mixin Tagged<M> {
  List<M> tags() => <M>[];
  bool isTag(Object? o) => o is M;
}

class WithMixin<P> extends Lone<List<P>> with Tagged<Map<P, int>> {}

abstract class Shape<S> {
  S get value;
  List<S> wrap() => [value];
}

class Circle<C extends num> implements Shape<C> {
  final C value;
  Circle(this.value);
  List<C> wrap() => [value];
}

class Box<V> extends Shape<V> {
  final V value;
  Box(this.value);
  void set(V v) {}
}

// Same rule body (only the Base supertype): shared by alias in production.
class P<T> extends Base<T, T> {}

class Q<T> extends Base<T, T> {}

void main() {
  final s = Swap<int, String>();
  print(s.firsts().runtimeType);
  print([s.pairs() is Map<String, int>, s.pairs() is Map<int, String>]);
  print([s.isFirst('a'), s.isFirst(1), s.isSecond('a'), s.isSecond(1)]);
  print(s.xs().runtimeType);
  print(s.own());
  print(s.describe());
  try {
    s.castSecond('no');
    print('cast passed');
  } on TypeError {
    print('cast failed');
  }
  print(s.castSecond(3));

  final f = Fixed();
  print([f.firsts().runtimeType, f.xs().runtimeType, f.own(), f.describe()]);

  final l = Lone<double>();
  print([l.make().runtimeType, l.test(1.5), l.test('x')]);
  print(l.identity() is double Function(double));
  print(l.identity() is int Function(int));

  final w = WithMixin<bool>();
  print([w.make().runtimeType, w.tags().runtimeType]);
  print([w.isTag(<bool, int>{}), w.isTag(<int, int>{}), w.test(<bool>[])]);

  print([Circle(2).wrap().runtimeType, Circle(2.5).wrap().runtimeType]);
  print(Box('b').wrap().runtimeType);
  dynamic b = Box<int>(1);
  try {
    b.set('x');
    print('dynamic set passed');
  } on TypeError {
    print('dynamic set failed');
  }
  b.set(2);

  final ints = <int>[1, 2, 3];
  print(ints.toList().runtimeType);
  print(ints.map((i) => '$i').toList().runtimeType);
  print(ints.whereType<int>().toList().runtimeType);
  print({'a': 1}.keys.toList().runtimeType);
  print({'a': 1}.values.toList().runtimeType);

  final p = P<int>();
  final q = Q<int>();
  print([p is Q<int>, q is P<int>, p is Base<int, int>, q is Base<int, int>]);
  print([p.firsts().runtimeType, q.pairs() is Map<int, int>, p.isSecond(1)]);
  print([<P<int>>[] is List<Q<int>>, <Q<int>>[] is List<P<int>>]);
}
