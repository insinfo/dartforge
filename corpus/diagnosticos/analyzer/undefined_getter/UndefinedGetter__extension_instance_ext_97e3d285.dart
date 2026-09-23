class C {
  void set foo(int _) {}
}

extension E on C {
  int get foo => 0;

  f() {
    this.foo;
//       ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'C'.
  }
}
