enum E(int x) {
  v(0);
  const E.named() : this(0);
//      ^^^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  this : this.named(), assert(x > -1);
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
