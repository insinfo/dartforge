enum E {
  v;
  const E() : this.foo(), this.bar();
//                        ^^^^^^^^^^
// [diag.multipleRedirectingConstructorInvocations] Constructors can have only one 'this' redirection, at most.
  const E.foo();
  const E.bar();
}
