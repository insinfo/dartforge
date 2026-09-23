abstract class A {
  factory A() => B();
}

class B implements A {}

void foo() {
  A.new;
}
