class A(this.v) {
//           ^
// [diag.fieldInitializedInDeclarationAndParameterOfPrimaryConstructor] Fields can't be initialized in both the primary constructor parameter list and at their declaration.
  int v = 0;
  this : v = 0;
//       ^
// [diag.fieldInitializedInParameterAndInitializer] Fields can't be initialized in both the parameter list and the initializers.
}
