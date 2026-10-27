// Type parameters left unconstrained are instantiated to their bounds, which
// may mention the enclosing class's or functions' type parameters: in a
// dynamic call of a generic method, in a call of a local generic function, in
// a partial tear-off instantiation and in a dynamic call of a generic closure.
typedef MapFunc<S1, S2> = void Function(Map<S1, S2>? arg);

class A<P> {
  final List seen = [];

  void foo<Q, Q1 extends P>() {
    void bar<T1 extends P, T2 extends Q>(Map<T1, T2>? arg) {
      seen
        ..add(T1)
        ..add(T2);
    }

    bar(null);
    MapFunc<Q1, Q> instantiated = bar;
    instantiated(null);
    void baz<T3 extends Map<Q1, P>>(List<T3>? arg) => seen.add(T3);
    dynamic f = baz;
    f(null);
  }
}

void main() {
  dynamic a = A<int>();
  a.foo();
  print(a.seen);
  var b = A<num>();
  b.foo<String, int>();
  print(b.seen);
}
