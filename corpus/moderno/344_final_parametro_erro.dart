// requer-dart: 3.13
// erro-de-compilacao
// Negativo: na 3.13, `final`/`var` só existem em parâmetro declarante de
// construtor primário (`Can't have modifier 'final' here`).
void f(final int x) => print(x);

void main() {
  f(1);
}
