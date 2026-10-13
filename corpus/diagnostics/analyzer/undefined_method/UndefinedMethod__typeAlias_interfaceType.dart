typedef A = List<int>;

void f() {
  A.foo();
//  ^^^
// [diag.undefinedMethodOnTypeLiteral] The method 'foo' isn't defined for the type 'List'.
}
