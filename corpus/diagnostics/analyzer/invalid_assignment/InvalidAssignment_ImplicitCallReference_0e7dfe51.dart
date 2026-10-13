class C {
  void call(int a) {}
}
class D<U extends Function> {
  U f = C();
//      ^^^
// [diag.invalidAssignment] A value of type 'C' can't be assigned to a variable of type 'U'.
}
