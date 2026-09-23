class A {}
class B extends A {
  const B(): super();
//           ^^^^^^^
// [diag.constConstructorWithNonConstSuper] A constant constructor can't call a non-constant super constructor of 'A'.
}
