typedef _NaoUsado = int;
typedef _Usado = int;
typedef void _Antigo();
typedef _SoLocal = String;
class _C {
  _C();
  _C.nomeado();
  _C._priv();
}
typedef Exposto = _C;
class _D {
  _D();
  _D.nomeado();
}
typedef _NaoExposto = _D;
_Usado f() {
  _SoLocal? s;
  print(s);
  return 0;
}
