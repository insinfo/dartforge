extension type A(int it) {
  A.named(this.it) : it = 0;
//                   ^^
// [diag.fieldInitializedInParameterAndInitializer] Fields can't be initialized in both the parameter list and the initializers.
}
