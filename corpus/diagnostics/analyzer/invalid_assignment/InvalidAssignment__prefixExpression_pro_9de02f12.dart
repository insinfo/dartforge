class A {
  A operator+(_) => this;
}

class C {
  A a = A();
}

f(C c) {
  ++c.a;
}
