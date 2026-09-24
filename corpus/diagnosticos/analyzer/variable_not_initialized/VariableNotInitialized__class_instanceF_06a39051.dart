class A {
  int v = 0;
  A(this.v) : v = 0 {}
//            ^
// [diag.fieldInitializedInParameterAndInitializer] Fields can't be initialized in both the parameter list and the initializers.
}
