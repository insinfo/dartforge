class A {
  A(int x) : assert(x > 0), this.name();
//           ^^^^^^^^^^^^^
// [diag.assertInRedirectingConstructor] A redirecting constructor can't have an 'assert' initializer.
  A.name() {}
}
