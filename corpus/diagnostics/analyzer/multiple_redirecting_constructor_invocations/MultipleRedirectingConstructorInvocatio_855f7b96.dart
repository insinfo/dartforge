class A {
  A() : this.foo(), this.bar();
//                  ^^^^^^^^^^
// [diag.multipleRedirectingConstructorInvocations] Constructors can have only one 'this' redirection, at most.
  A.foo() {}
  A.bar() {}
}
