void f(String? s) {
  s?.length?.isEven;
// ^^
// [context 1] The operator '?.' is causing the short circuiting.
//         ^^
// [diag.invalidNullAwareOperatorAfterShortCircuit][context 1] The receiver can't be 'null' because of short-circuiting, so the null-aware operator '?.' can't be used.
}
