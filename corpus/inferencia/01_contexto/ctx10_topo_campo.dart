// R-CTX-10: inferência de topo e de campos a partir do inicializador;
// herança de tipo de membro sobrescrito (override inference).
var lista = [1, 2.5];
final mapa = {'a': 1};

abstract class A {
  num valor(int x);
  List<num> get itens;
}

class B extends A {
  final campo = <String>[];
  valor(x) => /*@*/x;
  get itens => /*@*/[1];
}

void main() {
  print([/*@*/lista, /*@*/mapa, /*@*/B().campo, /*@*/B().valor(1), /*@*/B().itens]);
}
