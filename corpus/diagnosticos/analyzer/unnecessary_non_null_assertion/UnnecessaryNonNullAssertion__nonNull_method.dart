class A {
  static void foo() {}
}

void f() {
  A.foo!();
//     ^
// [diag.unnecessaryNonNullAssertion] The '!' will have no effect because the receiver can't be null.
}
