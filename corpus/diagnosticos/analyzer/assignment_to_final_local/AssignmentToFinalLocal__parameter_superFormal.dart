class A {
  A(int a);
}
class B extends A {
  var x;
  B(super.a) : x = (() { a = 0; });
//                       ^
// [diag.assignmentToFinalLocal] The final variable 'a' can only be set once.
}
