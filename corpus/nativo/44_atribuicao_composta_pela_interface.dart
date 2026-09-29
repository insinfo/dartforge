// Atribuição composta a um setter cujo getter vem de uma superinterface:
// `lista.last += x` (o `last` de `Iterable`, o `last=` de `List`), como o
// `TextPiece.append` do `dart_style`, e o mesmo numa hierarquia do programa.
abstract interface class Nomeado {
  String get nome;
}

abstract class ComSetter implements Nomeado {
  set nome(String v);
}

class Impl extends ComSetter {
  String _n = 'a';
  @override
  String get nome => _n;
  @override
  set nome(String v) => _n = v;
}

void main() {
  final linhas = <String>[''];
  linhas.last += 'x';
  linhas.add('');
  linhas.last += 'y';
  linhas.first += 'z';
  print(linhas);
  final numeros = [1, 2, 3];
  numeros.last *= 10;
  numeros.first -= 5;
  print(numeros);
  ComSetter c = Impl();
  c.nome += 'b';
  print(c.nome);
}
