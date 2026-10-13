void f(Never x) {
  x + (1 + 2);
//^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
}
