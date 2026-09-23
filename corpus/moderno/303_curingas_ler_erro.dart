// requer-dart: 3.13
// erro-de-compilacao
// Negativo: numa biblioteca 3.7+, ler `_` sem nenhum `_` que ligue nome é
// erro (`Undefined name '_'`).
void main() {
  var _ = 1;
  print(_);
}
