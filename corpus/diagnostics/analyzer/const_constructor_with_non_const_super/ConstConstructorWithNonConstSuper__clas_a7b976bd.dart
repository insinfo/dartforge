class A {}
class B extends A {
  const new();
//      ^^^
// [diag.constConstructorWithNonConstSuper] A constant constructor can't call a non-constant super constructor of 'A'.
}
