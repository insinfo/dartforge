class A {}
class const B() extends A {
  this : super();
//       ^^^^^^^
// [diag.constConstructorWithNonConstSuper] A constant constructor can't call a non-constant super constructor of 'A'.
}
