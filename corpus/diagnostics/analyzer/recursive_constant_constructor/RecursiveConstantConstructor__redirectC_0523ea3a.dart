class A {
  const new named() : this.named();
//                    ^^^^^^^^^^^^
// [diag.recursiveConstructorRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
