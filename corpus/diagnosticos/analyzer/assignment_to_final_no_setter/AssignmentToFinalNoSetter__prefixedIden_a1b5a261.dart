class A {
  int get x => 0;
}

void f(A a) {
  a.x = 0;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
  a.x += 0;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
  ++a.x;
//    ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
  a.x++;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
}
