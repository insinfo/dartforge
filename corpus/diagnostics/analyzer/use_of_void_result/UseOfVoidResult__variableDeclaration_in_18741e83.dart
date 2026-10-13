void f(void x) {
  // ignore:unused_local_variable
  dynamic v = x;
//            ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
