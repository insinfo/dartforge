class C {
  int get property => 0;
}

f(C c) {
  // Note: no diagnostic on the second `..property`.
  c?..property..property;
// ^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?..' is unnecessary.
}
