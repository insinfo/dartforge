// Exercita o gerador nos construtos que um projeto real usa: campos
// obrigatórios e opcionais, renomeados, aninhados, listas, mapas, enums,
// DateTime, valores padrão e chaves ignoradas.
import 'package:json_annotation/json_annotation.dart';

part 'modelos.g.dart';

enum Situacao { ativo, inativo, @JsonValue('suspenso_2') suspenso }

@JsonSerializable()
class Endereco {
  final String rua;
  final int numero;
  @JsonKey(name: 'cep_formatado')
  final String? cep;
  @JsonKey(defaultValue: '')
  final String complemento;
  Endereco({required this.rua, required this.numero, this.cep, this.complemento = ''});
  factory Endereco.fromJson(Map<String, dynamic> json) {
    // Corpo em bloco: a API não muda, só o corpo.
    return _$EnderecoFromJson(json);
  }
  Map<String, dynamic> toJson() => _$EnderecoToJson(this);
}

@JsonSerializable(explicitToJson: true, includeIfNull: false)
class Pessoa {
  final int id;
  final String nome;
  @JsonKey(defaultValue: Situacao.ativo)
  final Situacao situacao;
  final DateTime nascimento;
  final List<Endereco> enderecos;
  final Map<String, int> pontuacoes;
  @JsonKey(includeFromJson: false, includeToJson: false)
  final String segredo;
  final double? salario;
  Pessoa({
    required this.id,
    required this.nome,
    required this.situacao,
    required this.nascimento,
    required this.enderecos,
    required this.pontuacoes,
    this.segredo = 'x',
    this.salario,
  });
  factory Pessoa.fromJson(Map<String, dynamic> json) => _$PessoaFromJson(json);
  Map<String, dynamic> toJson() => _$PessoaToJson(this);
}
