class A {
  int get x => 0;
  set x(String _) {}
}

void f(A a) {
  a.x++;
//^^^^^
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type 'String'.
  a.x--;
//^^^^^
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type 'String'.
}
