// experimentos: macros
// @JsonCodable, @JsonEncodable e @JsonDecodable do package:json 0.20.4:
// aninhado, anulável, List/Set/Map, DateTime. A augmentation que o CFE 3.6.2
// gera está em esperado/ (o oráculo byte a byte do hospedeiro).
import 'package:caso_json/modelos.dart';

void main() {
  var u = Usuario.fromJson({
    'nome': 'Dart',
    'idade': 15,
    'notas': [1, 2],
    'tags': ['a', 'b', 'a'],
    'pesos': {'x': 1.5},
    'casa': {'rua': 'Principal', 'numero': 10},
    'outros': [
      {'rua': 'Segunda'}
    ],
    'criado': '2024-01-02T03:04:05.000',
    'ativo': true,
  });
  print(u.nome);
  print(u.tags);
  print(u.casa.rua);
  print(u.outros?.first.numero);
  print(u.toJson());
  print(SoSaida(2.5).toJson());
  print(SoEntrada.fromJson({'codigo': 'x9'}).codigo);
}
