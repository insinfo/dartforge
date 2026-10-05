class A {
  A();
  A._naoUsado();
  A._usado();
  A._porRedirecionamento();
  A._porSuper();
  A._porFabrica();
  A.r() : this._porRedirecionamento();
  factory A.f() = A._porFabrica;
}
class B extends A {
  B() : super._porSuper();
}
class Unico {
  Unico._();
}
class _Priv {
  _Priv.new();
  _Priv.n();
  _Priv.publicoNaoUsado();
}
class Tear {
  Tear();
  Tear._t();
}
void main() {
  A._usado();
  _Priv();
  _Priv.n();
  print(Tear._t);
}
