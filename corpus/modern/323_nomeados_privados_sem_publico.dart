// requer-dart: 3.13
// erro-de-compilacao
// Negativo: `__x` não tem nome público correspondente (tirar o `_` deixa
// outro nome privado).
class C {
  final int __x;
  C({required this.__x});
}

void main() {
  print(C(_x: 1).__x);
}
