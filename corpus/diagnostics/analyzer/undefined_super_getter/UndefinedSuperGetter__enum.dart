enum E {
  v;
  void f() {
    super.foo;
//        ^^^
// [diag.undefinedSuperGetter] The getter 'foo' isn't defined in a superclass of 'E'.
  }
}
