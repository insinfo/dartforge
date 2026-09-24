void f() {
  (throw '').hashCode;
//^^^^^^^^^^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
}
