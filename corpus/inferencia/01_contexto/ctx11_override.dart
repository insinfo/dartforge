// R-CTX-11: inferência de sobrescrita (override inference): parâmetros e
// retorno sem tipo herdam da assinatura do supertipo (inclusive de campo e
// de membro também inferido); parâmetro posicional casa por posição.
class A {
  void m(int a, [String? s]) {}
  final num campo = 1;
  var lista = [1.5];
}

class B extends A {
  @override
  void m(b, [t]) {
    print([/*@*/b, /*@*/t]);
  }

  @override
  get campo => /*@*/2;
  @override
  get lista => /*@*/[];
}

void main() {
  print([/*@*/B().campo, /*@*/B().lista]);
}
