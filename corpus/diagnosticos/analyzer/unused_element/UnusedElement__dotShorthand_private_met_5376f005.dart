class _C {
  static _C foo() => _C();
}

void main() {
  _C c;
  c = .foo();
  print(c);
}
