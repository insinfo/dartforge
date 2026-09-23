class A {
  dynamic f;
  A(void x) : f = x;
//                ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
