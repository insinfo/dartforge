class C {
  set foo(int _) {}
}

f(C c) {
  c.foo += 1;
//  ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'C'.
}
