void _naoUsada() {}
void _recursiva() { _recursiva(); }
void _a() { _b(); }
void _b() { _a(); }
void _tearoff() {}
/// Veja [_soDoc].
void _soDoc() {}
void _chamada() {}
void main() {
  var t = _tearoff;
  _chamada();
  print(t);
}
