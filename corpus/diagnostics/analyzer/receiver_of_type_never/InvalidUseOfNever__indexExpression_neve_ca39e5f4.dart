void f(Never x) {
  x?[0];
//^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?[' is unnecessary.
//   ^^^
// [diag.deadCode] Dead code.
}
