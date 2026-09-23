enum E() {
  v;
  final int x;
  const E.named() : this();
//      ^^^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  this : x = 0, this.named();
//              ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
