class A {
  int get x => 0;

  void f() {
    x = 0;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
    x += 0;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
    ++x;
//    ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
    x++;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
  }
}
