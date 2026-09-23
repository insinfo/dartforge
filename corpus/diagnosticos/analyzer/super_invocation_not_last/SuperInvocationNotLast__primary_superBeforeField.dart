class A() {
  int x;
  this : super(), x = 0;
//       ^^^^^
// [diag.superInvocationNotLast] The superconstructor call must be last in an initializer list.
}
