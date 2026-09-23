void f() {
  var (_ || _) = 0;
//     ^^^^^^
// [diag.refutablePatternInIrrefutableContext] Refutable patterns can't be used in an irrefutable context.
//       ^^^^
// [diag.deadCode] Dead code.
}
