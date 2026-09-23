class A {
  void foo() {}
}

void f(A a) {
  (a).foo = 0;
//    ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
  (a).foo += 1;
//    ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
//        ^^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'void Function()'.
  (a).foo++;
//    ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
  ++(a).foo;
//      ^^^
// [diag.assignmentToMethod] Methods can't be assigned a value.
}
