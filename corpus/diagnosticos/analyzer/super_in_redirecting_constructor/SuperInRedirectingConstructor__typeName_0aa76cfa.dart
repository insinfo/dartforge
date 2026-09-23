class A {
  A() : this.name(), super();
//                   ^^^^^^^
// [diag.superInRedirectingConstructor] The redirecting constructor can't have a 'super' initializer.
  A.name() {}
}
