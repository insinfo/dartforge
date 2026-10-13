extension E on int {
  void set foo(int _) {}
}
f() {
  0.foo;
//  ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'int'.
}
