class A() {
  int x;
  A.named() : this();
  this : x = 0, this.named();
//              ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
