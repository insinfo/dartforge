void _f1([int? a]) {}
void _f2([int? a]) {}
void _f3({int? a, int? b}) {}
void _f4<T>([int? a]) {}
void _f5([int? a]) {}
void pub([int? a]) {}
class _C {
  _C([int? a]);
  _C.n({int? a});
  void m([int? a]) {}
  static void s([int? a]) {}
}
class G<T> {
  G._([int? a]);
}
class S {
  S._(int a);
  S._o([int? a]);
}
class T extends S {
  T._([super.a]) : super._();
  T._o([super.a]) : super._o();
  T._p([int? a]) : super._(a ?? 0);
}
class Base {
  void _m([int? a]) {}
}
class Der extends Base {
  @override
  void _m([int? a, int? b]) {}
}
void main() {
  _f1();
  _f2(1);
  _f3(a: 1);
  _f4();
  print(_f5);
  _C();
  _C.n();
  _C.s();
  G._();
  T._();
  T._o();
  T._p();
  Base()._m(1);
  Der()._m();
}
