class A {
  num get foo => 0;
}

class B {
  int get foo => 0;
}

class C {
  void set foo(int _) {}
}

class D implements A, B, C {
  var foo = 0;
}
