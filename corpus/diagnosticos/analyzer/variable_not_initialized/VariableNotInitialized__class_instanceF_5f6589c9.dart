class A() {
  final int v = 0;
  this : v = 0;
//       ^
// [diag.fieldInitializedInDeclarationAndInitializerOfPrimaryConstructor] Fields can't be initialized in both the primary constructor and at their declaration.
}
