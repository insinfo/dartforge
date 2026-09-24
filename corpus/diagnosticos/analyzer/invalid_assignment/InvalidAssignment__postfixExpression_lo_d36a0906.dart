class A {
  B operator+(_) => new B();
}

class B {}

f(A a) {
  a++;
//^^^
// [diag.invalidAssignment] A value of type 'B' can't be assigned to a variable of type 'A'.
}
