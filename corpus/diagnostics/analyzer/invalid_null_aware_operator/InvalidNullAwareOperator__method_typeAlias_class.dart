class A {
  static void foo() {}
}

typedef B = A; 

f() {
  B?.foo();
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
