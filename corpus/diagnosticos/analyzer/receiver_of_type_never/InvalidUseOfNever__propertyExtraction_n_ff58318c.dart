typedef N = Never;

void f(N x) {
  (x).foo;
//^^^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
}
