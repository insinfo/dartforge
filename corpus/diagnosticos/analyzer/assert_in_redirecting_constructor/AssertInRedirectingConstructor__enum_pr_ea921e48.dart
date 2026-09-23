enum E(int x) {
  v(0);
  const E.named() : this(0);
//      ^^^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  this : assert(x > -1), this.named();
//                       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
