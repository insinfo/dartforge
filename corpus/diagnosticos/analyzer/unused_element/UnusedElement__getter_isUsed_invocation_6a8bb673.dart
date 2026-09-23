class A {
  get _g => null;
  useGetter() {
    var v = _g;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  }
}
