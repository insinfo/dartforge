class A {
  A(int x) : this.name(), assert(x > 0);
//                        ^^^^^^^^^^^^^
// [diag.assertInRedirectingConstructor] A redirecting constructor can't have an 'assert' initializer.
  A.name() {}
}
