abstract class A {
  late final int x = 0;
}

void f(A a) {
  (a).x = 0;
//    ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  (a).x += 0;
//    ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  ++(a).x;
//      ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  (a).x++;
//    ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
}
