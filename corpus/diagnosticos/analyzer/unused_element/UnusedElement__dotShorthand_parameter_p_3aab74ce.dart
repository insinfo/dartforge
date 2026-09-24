class A {
  final int? f;
  A([this.f]);
  factory A.named([int? a]) = A;
}
void main() {
  A a;
  a = .named(0);
  print(a);
}
