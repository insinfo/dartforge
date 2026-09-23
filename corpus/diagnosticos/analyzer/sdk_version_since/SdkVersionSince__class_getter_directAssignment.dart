import 'dart:foo';

void f(A a) {
  (a).foo = 0;
//    ^^^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'foo' in class 'A'.
}
