// @dart=3.9
// requer-dart: 3.13
// erro-de-compilacao
// Negativo: atalhos de ponto exigem 3.10.
enum Cor { vermelho, azul }

void main() {
  Cor c = .vermelho;
  print(c);
}
