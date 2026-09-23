// @dart=3.12
// requer-dart: 3.13
// Antes da 3.13, `factory() {}` é um MÉTODO chamado `factory`, e `final` em
// parâmetro comum é permitido.
class F {
  int factory() => 42;
}

void soma(final int a, var b) => print(a + b);

void main() {
  print(F().factory());
  soma(1, 2);
}
