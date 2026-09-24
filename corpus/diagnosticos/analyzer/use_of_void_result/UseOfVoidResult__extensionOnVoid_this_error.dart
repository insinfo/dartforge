extension on void {
  testVoid() {
//^^^^^^^^
// [diag.unusedElement] The declaration 'testVoid' isn't referenced.
    // No access on void. Static type of `this` is void!
    this.toString();
//  ^^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
  }
}
