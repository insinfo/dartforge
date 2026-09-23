class C {
  int Function() get g => () => 0;
}

f(C c) {
  c?..g().toString();
// ^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?..' is unnecessary.
}
