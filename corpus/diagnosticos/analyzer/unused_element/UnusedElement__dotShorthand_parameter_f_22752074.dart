class _A {
  final int? f;
  _A([this.f]);
  factory _A.named([int? a]) = _A;
}
void main() {
  _A a;
  a = .named(0);
  print(a);
}
