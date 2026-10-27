// Tearing off a generic method whose type parameter's bound mentions the
// class's type parameter checks the tear-off against its static type (the
// bound is an invariant position). A generic function type with an F-bounded
// type parameter out of scope has a finite runtime type.
class C<T> {
  void foo<S extends T>(S x) => print('foo $x');
}

class A<X> {}

typedef Fb = void Function<Y extends A<Y>>(Y);

void takes<Y extends A<Y>>(Y y) {}

class Holder<T extends A<T>> {
  List<void Function(T)> callbacks = [];
}

class B extends A<B> {}

void main() {
  C<Object> c = C<int>();
  try {
    void Function(String) fn = c.foo;
    print('no error ${fn.runtimeType}');
  } on TypeError {
    print('TypeError');
  }
  C<num> n = C<num>();
  void Function(int) ok = n.foo;
  ok(3);
  Fb f = takes;
  print(f is Fb);
  var h = Holder<B>();
  h.callbacks.add((b) => print('called'));
  h.callbacks.first(B());
  print(h.callbacks.runtimeType);
}
