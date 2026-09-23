enum E() {
  v;
  this : this.noSuchConstructor();
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
