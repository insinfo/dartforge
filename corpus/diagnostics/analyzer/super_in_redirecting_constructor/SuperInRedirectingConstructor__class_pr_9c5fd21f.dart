class A() {
  A.named() : this();
  this : this.named(), super();
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
