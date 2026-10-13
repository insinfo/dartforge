class A(int x) {
  A.named() : this(0);
  this : this.named(), assert(x > 0);
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
