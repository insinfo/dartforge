class C {
  void foo() {}
}

extension E on C {
  void set foo(int _) {}

  f() {
    this.foo = 0;
//       ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
  }
}
