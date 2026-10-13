void f(Object? x) {
  if (x case A(foo: _, bar: _?)) {}
  if (x case A(bar: _?, foo: _)) {}
}

class A {
  int get foo => 0;
  int? get bar => 0;
}
