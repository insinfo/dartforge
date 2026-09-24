class A(this.v) {
//           ^
// [diag.fieldInitializedInDeclarationAndParameterOfPrimaryConstructor] Fields can't be initialized in both the primary constructor parameter list and at their declaration.
  int v = 0;
}
