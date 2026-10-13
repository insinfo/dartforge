void f(Object? x) {
  if (x case A(_foo: var bar)) {
    bar;
  }
}

class A {
  int _foo = 0;
}
