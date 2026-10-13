typedef N = Never;

void f(N x) {
  x[0] += 1;
//^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
//  ^^^^^^^^
// [diag.deadCode] Dead code.
}
