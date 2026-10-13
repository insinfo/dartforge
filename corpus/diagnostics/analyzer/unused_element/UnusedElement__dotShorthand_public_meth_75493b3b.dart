class C {
  static C foo() => C();
}

void main() {
  C c;
  c = .foo();
  print(c);
}
