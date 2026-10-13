class A {
  int m() {
    return;
//  ^^^^^^
// [diag.returnWithoutValue] The return value is missing after 'return'.
  }
}
