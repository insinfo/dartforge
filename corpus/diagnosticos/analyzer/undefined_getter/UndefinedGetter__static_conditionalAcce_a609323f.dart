class A {
  static var x;
}
var a = A?.x;
//       ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
