class A {
  int operator [](int index) => 0;
  void operator []=(int index, String value) {}
}

void f(A a) {
  a[0]++;
//^^^^^^
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type 'String'.
  a[0]--;
//^^^^^^
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type 'String'.
}
