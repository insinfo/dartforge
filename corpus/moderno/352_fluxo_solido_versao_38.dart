// @dart=3.8
// requer-dart: 3.13
// erro-de-compilacao
// Negativo: antes da 3.9, `x != null` com `x` não anulável ainda pode ser
// falso para a análise de fluxo, e `y` não está definitivamente atribuída.
void main() {
  var x = 0;
  String y;
  if (x != null) y = 'definido';
  print(y);
}
