import 'dart:foo';

void f() {
  foo = 0;
//^^^
// [diag.assignmentToFinal] 'foo' can't be used as a setter because it's final.
}
