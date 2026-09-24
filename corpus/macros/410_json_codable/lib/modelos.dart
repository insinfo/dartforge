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
  final String? apelido;
  final List<int> notas;
  final Set<String> tags;
  final Map<String, double> pesos;
  final Endereco casa;
  final List<Endereco>? outros;
  final DateTime criado;
  final bool ativo;
}

@JsonEncodable()
class SoSaida {
  final num valor;
  SoSaida(this.valor);
}

@JsonDecodable()
class SoEntrada {
  final String codigo;
}
