// requer-dart: 3.13
// erro-de-compilacao
// Negativo: `super._b` não tem nome público a encaminhar (o parâmetro do
// super é `b`); só `this._x` e parâmetro declarante podem ser privados.
class A {
  final int _b;
  A({required this._b});
}

class B extends A {
  B({required super._b});
}

void main() {
  print(B(b: 1)._b);
}
