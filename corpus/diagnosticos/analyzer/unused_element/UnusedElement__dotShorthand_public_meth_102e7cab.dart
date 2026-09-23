class C {
  static C foo({int? p}) => C();
}
void main() {
  C c;
  c = .foo(p: 0);
  print(c);
}
