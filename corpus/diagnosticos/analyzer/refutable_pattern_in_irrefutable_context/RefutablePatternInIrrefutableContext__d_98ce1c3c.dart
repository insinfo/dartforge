void f(int? x) {
  var (_?) = x;
//     ^^
// [diag.refutablePatternInIrrefutableContext] Refutable patterns can't be used in an irrefutable context.
}
