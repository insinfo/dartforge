class G<T> {
  const G();
}
U id<U>(U u) => u;
class K<T> {
  final a = G<T>.new;
  final b = id<T>;
  final int Function(int) b2 = id;
  final T Function(T) b3 = id;
  final c = const G<T>();
  final d = G<T>();
  static final e = id<int>;
  const K();
  void m({Object p = G<T>.new, Object q = id<T>, Object r = const G<T>()}) {}
}
class L<T> {
  final a = G<T>.new;
  final b = id<T>;
  final T Function(T) b3 = id;
  L();
  const factory L.f() = L2<T>;
}
class L2<T> implements L<T> {
  const L2();
  T Function(T) get b3 => id;
  get a => 0;
  get b => 0;
}
enum En<T> {
  v<int>();
  final a = G<T>.new;
  const En();
}
