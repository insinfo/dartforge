class A(int x) {
  A.named() : this(0);
  this : assert(x > 0), this.named();
//                      ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
