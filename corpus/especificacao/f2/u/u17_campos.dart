class A {
  int _nunca = 0;
  int _soEscrito = 0;
  int _lido = 0;
  int _inc = 0;
  int _incThis = 0;
  int _composto = 0;
  int? _seNulo;
  int _incEmExpr = 0;
  final int _soInicializador;
  final int _soFormal;
  int _padrao = 0;
  int _relacional = 0;
  static int _estatico = 0;
  int _viaOutro = 0;
  A(this._soFormal) : _soInicializador = 1;
  void m(A o) {
    _soEscrito = 1;
    print(_lido);
    _inc++;
    this._incThis++;
    _composto += 1;
    _seNulo ??= 1;
    print(_incEmExpr++);
    o._viaOutro = 2;
    if (o case A(_padrao: 1)) {}
  }
}
