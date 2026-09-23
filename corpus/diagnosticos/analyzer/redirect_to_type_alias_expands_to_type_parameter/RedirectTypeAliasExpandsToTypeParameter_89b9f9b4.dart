class A implements C {}

typedef B = A;

class C {
  factory C() = B;
}
