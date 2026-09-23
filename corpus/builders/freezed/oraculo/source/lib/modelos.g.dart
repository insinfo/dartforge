// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'modelos.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

_Endereco _$EnderecoFromJson(Map<String, dynamic> json) => _Endereco(
      rua: json['rua'] as String,
      numero: (json['numero'] as num?)?.toInt() ?? 0,
    );

Map<String, dynamic> _$EnderecoToJson(_Endereco instance) => <String, dynamic>{
      'rua': instance.rua,
      'numero': instance.numero,
    };

_Pessoa _$PessoaFromJson(Map<String, dynamic> json) => _Pessoa(
      id: (json['id'] as num).toInt(),
      nome: json['nome'] as String,
      email: json['e_mail'] as String?,
      apelidos: (json['apelidos'] as List<dynamic>?)
              ?.map((e) => e as String)
              .toList() ??
          const <String>[],
      endereco: json['endereco'] == null
          ? null
          : Endereco.fromJson(json['endereco'] as Map<String, dynamic>),
    );

Map<String, dynamic> _$PessoaToJson(_Pessoa instance) => <String, dynamic>{
      'id': instance.id,
      'nome': instance.nome,
      'e_mail': instance.email,
      'apelidos': instance.apelidos,
      'endereco': instance.endereco,
    };

Circulo _$CirculoFromJson(Map<String, dynamic> json) => Circulo(
      raio: (json['raio'] as num).toDouble(),
      $type: json['tipo'] as String?,
    );

Map<String, dynamic> _$CirculoToJson(Circulo instance) => <String, dynamic>{
      'raio': instance.raio,
      'tipo': instance.$type,
    };

Retangulo _$RetanguloFromJson(Map<String, dynamic> json) => Retangulo(
      largura: (json['largura'] as num).toDouble(),
      altura: (json['altura'] as num).toDouble(),
      $type: json['tipo'] as String?,
    );

Map<String, dynamic> _$RetanguloToJson(Retangulo instance) => <String, dynamic>{
      'largura': instance.largura,
      'altura': instance.altura,
      'tipo': instance.$type,
    };
