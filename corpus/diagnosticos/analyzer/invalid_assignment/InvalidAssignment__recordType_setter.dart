void f(C c) {
  c.r = (a: 1, b: 2);
//      ^^^^^^^^^^^^
// [diag.invalidAssignment] A value of type '({int a, int b})' can't be assigned to a variable of type '(int, int)?'.
}
class C {
  (int, int)? r;
}
