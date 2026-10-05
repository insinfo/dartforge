class _P {
  void instancia() {}
  static void estatico() {}
  static void estaticoUsado() {}
  static int campoEstatico = 0;
  static int campoEstaticoLido = 0;
  int campoInstancia = 0;
  _P();
  _P.nomeado();
  _P.usado();
}
mixin _M {
  static void s() {}
  void i() {}
}
class Q with _M {}
void main() {
  _P.estaticoUsado();
  print(_P.campoEstaticoLido);
  _P.usado();
}
