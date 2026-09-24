abstract class Repository<T> {
  factory Repository() = Memory<T>;
}

class Memory<T> implements Repository<T> {
  Memory();
}

abstract class Pair<A, B> {
  factory Pair(A a, B b) = PairImpl<B, A>.flipped;
}

class PairImpl<X, Y> implements Pair<Y, X> {
  final Y a;
  final X b;
  PairImpl.flipped(this.a, this.b);
}

void main() {
  final r = Repository<String>();
  print(r is Memory<String>);
  print(r.runtimeType);
  final p = Pair<int, String>(3, 'x');
  print(p is PairImpl<String, int>);
  print(p.runtimeType);
}
