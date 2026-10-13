class _C {
  const _C.named({int? p});
}
void main() {
  _C c;
  c = const .named(p: 0);
  print(c);
}
