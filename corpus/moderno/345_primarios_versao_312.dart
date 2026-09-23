// @dart=3.12
// requer-dart: 3.13
// erro-de-compilacao
// Negativo: construtor primário exige 3.13.
class Point(var int x, var int y);

void main() {
  print(Point(1, 2).x);
}
