class A {
  factory A() => A.two();

  A.two();
}

void foo() {
  A.new;
}
