dynamic f(void x) sync* {
  yield x;
//      ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
