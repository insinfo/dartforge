class A {
  m() async {
    int async;
//      ^^^^^
// [diag.unusedLocalVariable] The value of the local variable 'async' isn't used.
  }
}
