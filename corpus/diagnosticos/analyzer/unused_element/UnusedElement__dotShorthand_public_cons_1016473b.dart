class C {
  const C.named({int? p});
}
void main() {
  C c;
  c = const .named(p: 0);
  print(c);
}
