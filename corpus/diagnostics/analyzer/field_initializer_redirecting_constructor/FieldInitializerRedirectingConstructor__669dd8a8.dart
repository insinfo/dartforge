enum E() {
  v;
  final int x;
  const E.named() : this();
//      ^^^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  this : this.named(), x = 0;
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
