extension type _NaoUsado(int v) {}
extension type _Usado(int v) {}
extension type _SoTipoLocal(int v) {}
extension type Pub._(int _v) {
  Pub.outro(this._v);
  Pub._priv(this._v);
  void _m() {}
}
extension type _Dois(int v) {
  _Dois.n(this.v);
}
void main() {
  print(_Usado(1));
  _SoTipoLocal? x;
  print(x);
  print(_Dois.n(1));
}
