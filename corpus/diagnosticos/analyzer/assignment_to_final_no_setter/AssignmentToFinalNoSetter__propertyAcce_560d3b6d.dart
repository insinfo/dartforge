extension E on int {
  int get x => 0;
}

void f() {
  0.x = 0;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'E'.
  0.x += 0;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'E'.
  ++0.x;
//    ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'E'.
  0.x++;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'E'.
}
