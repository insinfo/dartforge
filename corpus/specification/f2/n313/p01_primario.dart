class A(final int _naoLido, var int _lido, final int publico, int _soParametro) {
  int get g => _lido;
}
class B(final int _x) {
  void m() { _x; }
}
class C(var int _y) {
  void m() { _y = 1; _y++; }
}
enum E(final int _v, final int _w) {
  a(1, 2);
  int get w => _w;
}
extension type T(int _r) {}
class D(final int _, final int __) {}
