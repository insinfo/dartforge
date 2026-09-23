void f(Never x) {
  x[0];
//^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
//  ^^^
// [diag.deadCode] Dead code.
}
