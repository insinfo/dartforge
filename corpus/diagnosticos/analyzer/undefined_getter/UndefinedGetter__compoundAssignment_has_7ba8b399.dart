class C {
  static set foo(int _) {}
}

f() {
  C.foo += 1;
//  ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'C'.
}
