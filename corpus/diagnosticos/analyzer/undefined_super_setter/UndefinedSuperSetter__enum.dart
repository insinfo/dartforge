enum E {
  v;
  void f() {
    super.foo = 0;
//        ^^^
// [diag.undefinedSuperSetter] The setter 'foo' isn't defined in a superclass of 'E'.
  }
}
