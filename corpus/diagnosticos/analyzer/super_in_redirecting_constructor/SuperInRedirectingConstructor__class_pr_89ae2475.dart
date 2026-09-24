class A() {
  A.named() : this();
  this : super(), this.named();
//                ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
