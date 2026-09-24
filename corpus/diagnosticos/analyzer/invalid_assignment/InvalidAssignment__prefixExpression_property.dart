class A {
  B operator+(_) => new B();
}

class B {}

class C {
  A a = A();
}

f(C c) {
  ++c.a;
//^^^^^
// [diag.invalidAssignment] A value of type 'B' can't be assigned to a variable of type 'A'.
}
