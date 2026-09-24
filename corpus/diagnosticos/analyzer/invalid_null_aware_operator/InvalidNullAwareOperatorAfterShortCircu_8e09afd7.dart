void f(String? s) {
  s?.substring(0, 5)?.toLowerCase()?.length;
// ^^
// [context 1] The operator '?.' is causing the short circuiting.
// [context 2] The operator '?.' is causing the short circuiting.
//                  ^^
// [diag.invalidNullAwareOperatorAfterShortCircuit][context 1] The receiver can't be 'null' because of short-circuiting, so the null-aware operator '?.' can't be used.
//                                 ^^
// [diag.invalidNullAwareOperatorAfterShortCircuit][context 2] The receiver can't be 'null' because of short-circuiting, so the null-aware operator '?.' can't be used.
}
