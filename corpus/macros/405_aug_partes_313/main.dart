// requer-dart: 3.13
// experimentos: augmentations,enhanced-parts
// A forma atual da spec (v1.46): augmentations em *parts* com imports
// próprios, contra o 3.13.4. Parte dentro de parte (a pré-ordem da árvore
// dá a ordem dos inicializadores de campo: 1 2 3), `implements` acrescentado
// (os dois CFEs, 3.6.2 e 3.13.4, o ignoram no `is`: não conferido) e o texto
// fundido que o CFE 3.6.2 gera para o `@JsonCodable`, como está, numa parte.
// (O 3.13.4 não liga augmentation de mixin à origem nem aplica `with` vindo
// de augmentation: sem oráculo, ficam fora; o mixin aumentado está no 403.)
part 'json_macro.dart';
part 'clausulas.dart';

int i = 0;
int proximo(String quem) {
  i++;
  print('$quem = $i');
  return i;
}

abstract interface class Descrevivel {
  String descrever();
}

class Usuario {
  final String nome;
  final int idade;
  final String? apelido;
  final List<int> notas;
}

class C {
  int f1 = proximo('f1');
  external String rotulo();
}

external int dobro(int x);

void main() {
  var u = Usuario.fromJson({'nome': 'Dart', 'idade': 15, 'notas': [1, 2]});
  print(u.nome);
  print(u.toJson());
  var c = C();
  print('${c.f1} ${c.f2} ${c.f3}');
  print(c.descrever());
  print(c.rotulo());
  print(dobro(21));
}
