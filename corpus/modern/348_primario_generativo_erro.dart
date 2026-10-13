// requer-dart: 3.13
// erro-de-compilacao
// Negativo: com construtor primário, todo construtor generativo do corpo tem
// de redirecionar (senão os inicializadores de campo não veriam os
// parâmetros do primário).
class C(final int x) {
  C.dobro(int y) : this(y * 2);
  C.errado() : x = 0;
}

void main() {
  print(C.dobro(2).x);
}
