class C {
  int operator[](int index) => 0;
}

f(C c) {
  // Note: no diagnostic on the second `..[0]`.
  c?..[0]..[0];
// ^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?..' is unnecessary.
}
