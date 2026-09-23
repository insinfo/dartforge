class A {
  final int x;
  A() : super(), x = 1;
//      ^^^^^
// [diag.superInvocationNotLast] The superconstructor call must be last in an initializer list.
}
