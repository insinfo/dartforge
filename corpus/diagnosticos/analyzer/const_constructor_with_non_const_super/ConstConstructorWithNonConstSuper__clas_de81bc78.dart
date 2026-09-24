class A {
  factory A() = A._;
  const A._();
}

class B extends A {
  const B.foo() : this.bar();
  const B.bar() : super._();
}
