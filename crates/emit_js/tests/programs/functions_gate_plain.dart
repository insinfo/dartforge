// No closure, tear-off or function type anywhere: the production rti
// keeps no function kinds, and every type test matches the VM.

class A {}

class B extends A {}

class G<T> {
  bool isT(Object? o) => o is T;
  List<T> make() => <T>[];
}

void main() {
  final values = <Object?>[A(), B(), 1, 'x', null, <int>[], G<int>()];
  for (final v in values) {
    print([v is A, v is B, v is int, v is List<int>, v is G<int>, v is G<num>]);
  }
  print([G<A>().isT(B()), G<B>().isT(A()), G<int?>().isT(null)]);
  print([G<String>().make().runtimeType, G<List<int>>().make().runtimeType]);
  try {
    (values[0] as dynamic) as B;
  } on TypeError {
    print('cast failed');
  }
  print(StringBuffer('x')..write(2));
}
