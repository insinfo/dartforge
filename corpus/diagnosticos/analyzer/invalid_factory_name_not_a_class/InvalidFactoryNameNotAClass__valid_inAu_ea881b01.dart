class A {}

class B implements A {
  const B();
}

augment class A {
  const factory A() = B;
}
