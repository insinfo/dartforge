class _C {
  static _C foo({int? p}) => _C();
}
void main() {
  _C c;
  c = .foo(p: 0);
  print(c);
}
