void main() {
  void naoUsada() {}
  void recursiva() { recursiva(); }
  void usada() {}
  void _() {}
  void __() {}
  void a() { void b() {} }
  var c = () {};
  usada();
  a();
}
