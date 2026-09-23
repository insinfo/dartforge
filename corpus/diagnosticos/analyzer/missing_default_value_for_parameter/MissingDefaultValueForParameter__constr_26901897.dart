class A {
  factory A([int a]) = B;
}

class B implements A {
  B([int a = 0]);
}
