class A() {
  int x;
  A.named() : this();
  this : this.named(), x = 0;
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
