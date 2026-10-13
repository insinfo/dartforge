enum A(this.v) {
  e(0);
  final int v;
  this : v = 0;
//       ^
// [diag.fieldInitializedInParameterAndInitializer] Fields can't be initialized in both the parameter list and the initializers.
}
