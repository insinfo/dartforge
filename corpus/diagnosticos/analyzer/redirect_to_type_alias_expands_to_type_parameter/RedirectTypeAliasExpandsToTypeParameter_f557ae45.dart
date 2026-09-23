class A implements C {
  A.named();
}

typedef B = A;

class C {
  factory C() = B.named;
}
