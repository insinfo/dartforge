enum E {
  v(42);
  const E(int x) : assert(x > 0), this.name();
//                 ^^^^^^^^^^^^^
// [diag.assertInRedirectingConstructor] A redirecting constructor can't have an 'assert' initializer.
  const E.name();
}
