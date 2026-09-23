class A {
  int set x(int v) {
//^^^
// [diag.nonVoidReturnForSetter] The return type of the setter must be 'void' or absent.
    return 42;
  }
}