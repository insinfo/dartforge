// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'modelos.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

Endereco _$EnderecoFromJson(Map<String, dynamic> json) => Endereco(
      rua: json['rua'] as String,
      numero: (json['numero'] as num).toInt(),
      cep: json['cep_formatado'] as String?,
    );

Map<String, dynamic> _$EnderecoToJson(Endereco instance) => <String, dynamic>{
      'rua': instance.rua,
      'numero': instance.numero,
      'cep_formatado': instance.cep,
    };

Pessoa _$PessoaFromJson(Map<String, dynamic> json) => Pessoa(
      id: (json['id'] as num).toInt(),
      nome: json['nome'] as String,
      situacao: $enumDecodeNullable(_$SituacaoEnumMap, json['situacao']) ??
          Situacao.ativo,
      nascimento: DateTime.parse(json['nascimento'] as String),
      enderecos: (json['enderecos'] as List<dynamic>)
          .map((e) => Endereco.fromJson(e as Map<String, dynamic>))
          .toList(),
      pontuacoes: Map<String, int>.from(json['pontuacoes'] as Map),
      salario: (json['salario'] as num?)?.toDouble(),
    );

Map<String, dynamic> _$PessoaToJson(Pessoa instance) => <String, dynamic>{
      'id': instance.id,
      'nome': instance.nome,
      'situacao': _$SituacaoEnumMap[instance.situacao]!,
      'nascimento': instance.nascimento.toIso8601String(),
      'enderecos': instance.enderecos.map((e) => e.toJson()).toList(),
      'pontuacoes': instance.pontuacoes,
      if (instance.salario case final value?) 'salario': value,
    };

const _$SituacaoEnumMap = {
  Situacao.ativo: 'ativo',
  Situacao.inativo: 'inativo',
  Situacao.suspenso: 'suspenso_2',
};
