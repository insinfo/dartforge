// @dart=3.11
// requer-dart: 3.13
// erro-de-compilacao
// Negativo: numa biblioteca 3.11, nomeado privado é erro mesmo com `this.`.
class Pt {
  final int _x;
  Pt({required this._x});
}

void main() {
  print(Pt(x: 1)._x);
}
