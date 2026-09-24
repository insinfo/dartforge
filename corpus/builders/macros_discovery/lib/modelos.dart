import 'package:json/json.dart';

@JsonCodable()
class Endereco {
  final String rua;
  final int? numero;
}

@JsonCodable()
class Usuario {
  final String nome;
  final int idade;
  final Endereco casa;
}

@JsonEncodable()
class SoSaida {
  final num valor;
}

@JsonDecodable()
class SoEntrada {
  final String codigo;
}
