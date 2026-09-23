void f() {
  (throw '').toString;
//^^^^^^^^^^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
}
