class A<X> {
  final Object x1, x3, x5, x7, x8, x9;
  const A()
    : x1 = const [X],
      x3 = const {X},
      x5 = const {X: null},
      x7 = X,
      x8 = const B<X>(),
      x9 = <X>[];
  void m() {
    const [false is void Function(X)];
    const A<void Function<U extends X>()>();
    const [X];
    const y = X;
    const z = (X, 1);
    print([y, z]);
  }
}
class B<T> { const B(); }
