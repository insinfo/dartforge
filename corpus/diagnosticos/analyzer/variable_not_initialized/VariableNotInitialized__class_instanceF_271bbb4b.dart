class A {
  final int v = 0;
  A(this.v) {}
//       ^
// [diag.finalInitializedInDeclarationAndConstructor] 'v' is final and was given a value when it was declared, so it can't be set to a new value.
}
