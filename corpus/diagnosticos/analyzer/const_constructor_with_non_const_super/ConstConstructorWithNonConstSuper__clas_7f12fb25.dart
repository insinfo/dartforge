class A {
  factory A() = A._;
  A._();
}

class B extends A {
  const B.foo() : this.bar();
  const B.bar() : super._();
//                ^^^^^^^^^
// [diag.constConstructorWithNonConstSuper] A constant constructor can't call a non-constant super constructor of 'A'.
}
