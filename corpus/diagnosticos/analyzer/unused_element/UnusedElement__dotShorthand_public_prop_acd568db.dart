class C {
  static C a = C();
}

void main() {
  C c;
  c = .a;
  print(c);
}
