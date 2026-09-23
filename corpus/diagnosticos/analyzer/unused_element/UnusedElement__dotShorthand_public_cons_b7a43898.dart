class C {
  C.named({int? p});
}
void main() {
  C c;
  c = .named(p: 0);
  print(c);
}
