enum E {
  v(42);
  const E(int x) : this.name(), assert(x > 0);
//                              ^^^^^^^^^^^^^
// [diag.assertInRedirectingConstructor] A redirecting constructor can't have an 'assert' initializer.
  const E.name();
}
