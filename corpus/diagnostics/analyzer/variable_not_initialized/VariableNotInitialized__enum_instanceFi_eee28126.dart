enum A {
  e(0);
  final int v = 0;
  const A(this.v);
//             ^
// [diag.finalInitializedInDeclarationAndConstructor] 'v' is final and was given a value when it was declared, so it can't be set to a new value.
}
