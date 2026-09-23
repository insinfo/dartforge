class A {}

class B() extends A {
  B.foo() : this();
  B.bar() : this();
  this : this.foo(), this.bar();
//       ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
//                   ^^^^
// [diag.primaryConstructorCannotRedirect] A primary constructor can't be a redirecting constructor.
}
