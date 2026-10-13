// @dart=3.7
// requer-dart: 3.13
// erro-de-compilacao
// Negativo: elementos null-aware exigem 3.8; numa biblioteca 3.7 são erro.
void main() {
  int? x;
  print([?x, 1]);
}
