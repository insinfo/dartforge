class C {
  int method() => 0;
}

f(C c) {
  // Note: no diagnostic on the second `..method()`.
  c?..method()..method();
// ^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?..' is unnecessary.
}
