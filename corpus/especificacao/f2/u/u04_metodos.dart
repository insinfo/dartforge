class A {
  void _naoUsado() {}
  void _usadoPorThis() {}
  void _soDinamico() {}
  int _campoDinamico = 0;
  void _recursivo() { _recursivo(); }
  static void _estatico() {}
  void _tearoff() {}
  void m() {
    this._usadoPorThis();
    print(_tearoff);
  }
}
void f(dynamic d) {
  d._soDinamico();
  print(d._campoDinamico);
}
