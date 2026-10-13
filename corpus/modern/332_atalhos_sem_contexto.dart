// requer-dart: 3.13
// erro-de-compilacao
// Negativo: sem tipo de contexto que denote uma declaração, `.zero` não tem
// onde procurar (`var` não dá contexto).
void main() {
  var v = .zero;
  print(v);
}
