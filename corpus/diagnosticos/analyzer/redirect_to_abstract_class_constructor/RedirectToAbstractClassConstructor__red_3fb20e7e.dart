class A {
  factory A.named() = C;
  A();
}

class B extends A {}
typedef C = B;
