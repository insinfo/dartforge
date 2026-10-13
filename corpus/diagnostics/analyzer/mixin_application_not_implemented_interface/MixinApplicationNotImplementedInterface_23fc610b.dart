class A {
  void foo() {}
}

mixin M on A {
  void bar() {
    super.foo();
  }
}

class C {
  noSuchMethod(_) {}
}

class X = C with M;
//               ^
// [diag.mixinApplicationNotImplementedInterface] 'M' can't be mixed onto 'C' because 'C' doesn't implement 'A'.
