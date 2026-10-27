// A chain of redirecting factories creates the effective target with the
// type arguments composed along the chain. An assignment to a promoted local
// takes the declared type as context.
class C {
  const factory C() = D<int>;
}

class D<T> implements C {
  const factory D() = E<double, T>;
}

class E<S, T> implements D<T> {
  final field = 42;
  const E();
}

class X<T> {
  factory X() = Y<T>;
}

class Y<S> implements X<S> {
  Y();
}

void main() {
  var c = C();
  print([c is D<int>, c is E<double, int>, c is E<String, int>]);
  print(identical(const C(), const C()));
  var x = X<String>();
  print([x is Y<String>, x is Y<int>]);
}
