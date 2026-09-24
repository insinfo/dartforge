abstract class A {
  static late final int x = 0;
}

void f() {
  A.x = 0;
//  ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  A.x += 0;
//  ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  ++A.x;
//    ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  A.x++;
//  ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
}
