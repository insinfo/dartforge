// requer-dart: 3.13
// erro-de-compilacao
// Negativo: a parte `this { … }` só existe com construtor primário no
// cabeçalho.
class C {
  int x = 0;
  this {
    x = 1;
  }
}

void main() {
  print(C().x);
}
