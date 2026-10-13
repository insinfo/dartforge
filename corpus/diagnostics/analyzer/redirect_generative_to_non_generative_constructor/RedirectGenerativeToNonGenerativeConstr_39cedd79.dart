class A() {
  this : this.x();
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
  factory A.x() => throw 0;
}
