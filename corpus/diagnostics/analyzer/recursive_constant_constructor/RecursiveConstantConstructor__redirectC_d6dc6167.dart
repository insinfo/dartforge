class A {
  const new () : this();
//               ^^^^^^
// [diag.recursiveConstructorRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
