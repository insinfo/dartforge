class A {
  int x = 0;
  const factory A() = B;
}

class B implements A {
  const B();
  int get x => 0;
  void set x(_) {}
}
