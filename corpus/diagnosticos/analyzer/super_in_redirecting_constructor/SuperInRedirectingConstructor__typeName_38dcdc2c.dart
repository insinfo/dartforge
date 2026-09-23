class A {
  A() : super(), this.name();
//      ^^^^^^^
// [diag.superInRedirectingConstructor] The redirecting constructor can't have a 'super' initializer.
  A.name() {}
}
