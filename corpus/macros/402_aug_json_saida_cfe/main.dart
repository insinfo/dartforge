// experimentos: macros
// O texto que o CFE 3.6.2 gera para o `@JsonCodable` do package:json 0.20.4
// (extraído do .dill por scripts/oraculo_augmentation.dart), escrito à mão.
// O CFE aplica cada fase numa biblioteca de augmentation própria — as
// declarações da fase 2 antes das definições da fase 3 —, e o texto fundido
// que ele mostra só vale como um arquivo na forma nova (`part of`, 405).
import augment 'main_fase2.dart';
import augment 'main_fase3.dart';

class Usuario {
  final String nome;
  final int idade;
  final String? apelido;
  final List<int> notas;
}

void main() {
  var u = Usuario.fromJson({'nome': 'Dart', 'idade': 15, 'notas': [1, 2]});
  print(u.nome);
  print(u.toJson());
  var v = Usuario.fromJson({'nome': 'Rust', 'idade': 11, 'apelido': 'ferrugem', 'notas': []});
  print(v.toJson());
}
