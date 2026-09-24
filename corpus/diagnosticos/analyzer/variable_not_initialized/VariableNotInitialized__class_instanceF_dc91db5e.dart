class A {
  int v;
  A(this.v) : v = 0 {}
//            ^
// [diag.fieldInitializedInParameterAndInitializer] Fields can't be initialized in both the parameter list and the initializers.
}
