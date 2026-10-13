typedef A<T> = T Function();

void f(A<int> a) {
  A<String> b = a;
//          ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
//              ^
// [diag.invalidAssignment] A value of type 'A<int>' can't be assigned to a variable of type 'A<String>'.
}
