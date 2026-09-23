class C {
  void foo() {}
}

extension E on C {
  void set foo(int _) {}
}

void f(C c) {
  c.foo = 0;
//  ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
  c.foo += 1;
//  ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
  c.foo++;
//  ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
  --c.foo;
//    ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
}
