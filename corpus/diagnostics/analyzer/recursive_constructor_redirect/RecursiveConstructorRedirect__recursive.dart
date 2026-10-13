class A {
  A.a() : this.b();
//        ^^^^^^^^
// [diag.recursiveConstructorRedirect] Constructors can't redirect to themselves either directly or indirectly.
  A.b() : this.a();
//        ^^^^^^^^
// [diag.recursiveConstructorRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
