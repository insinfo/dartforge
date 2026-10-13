class A {
  factory A() {
    return;
//  ^^^^^^
// [diag.returnWithoutValue] The return value is missing after 'return'.
  }
}
