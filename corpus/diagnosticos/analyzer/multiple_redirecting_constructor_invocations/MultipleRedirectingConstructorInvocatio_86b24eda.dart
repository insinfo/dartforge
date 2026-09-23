enum E() {
  v;
  const E.foo() : this();
//      ^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  const E.bar() : this();
//      ^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  this : this.foo(), this.bar();
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
//                   ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
