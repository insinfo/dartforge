void f(Object? x) {
  if (x case A(_foo: var bar)) {
    bar;
  }
}

class A {
  int get _foo => 0;
}
