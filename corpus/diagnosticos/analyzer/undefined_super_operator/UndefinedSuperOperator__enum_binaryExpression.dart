enum E {
  v;
  void f() {
    super + 0;
//        ^
// [diag.undefinedSuperOperator] The operator '+' isn't defined in a superclass of 'E'.
  }
}
