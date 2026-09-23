class A {
  A(int? x) : super(), assert(x != null);
//            ^^^^^
// [diag.superInvocationNotLast] The superconstructor call must be last in an initializer list.
}
