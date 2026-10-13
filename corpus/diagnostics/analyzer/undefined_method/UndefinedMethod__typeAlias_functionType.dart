typedef A = void Function();

void f() {
  A.foo();
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'Type'.
}
