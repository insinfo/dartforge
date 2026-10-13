mixin M1 {
  final int a = 0;
}
mixin M2 {
  int b = 0;
  late final int c;
}
mixin M3 {
  abstract final int d;
  static int s = 0;
  int get g => 0;
}
mixin M4 {
  abstract int e;
}
class S {
  const S();
}
class X1 extends S with M1 {
  const X1();
}
class X2 extends S with M1, M2 {
  const X2.nome();
}
class X3 extends S with M3 {
  const X3();
}
class X4 extends S with M4 {
  const X4();
  const factory X4.f() = X4;
}
